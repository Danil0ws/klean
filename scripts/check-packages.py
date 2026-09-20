#!/usr/bin/env python3
"""Run the package-manager generation blocks from the release workflows locally,
with curl/sha256sum/git stubbed, then validate the artifacts they produce."""
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parents[1]
VERSION = "v1.2.3"
DOT = "1.2.3"
ASSET = f"v{DOT}/klean-v{DOT}-x86_64-pc-windows-msvc.zip"

STUBS = {
    "curl": """#!/usr/bin/env bash
# stub: pretend the release asset downloaded
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
[ -n "$out" ] && printf 'klean-%s' "$*" > "$out"
exit 0
""",
    "sha256sum": """#!/usr/bin/env bash
for f in "$@"; do printf '%s  %s\\n' 1111111111111111111111111111111111111111111111111111111111111111 "$f"; done
""",
    "git": """#!/usr/bin/env bash
echo "git $*" >> "${GIT_LOG:-/dev/null}"
if [ "$1" = "clone" ]; then
  # emulate: git clone <url> [dir]
  for last in "$@"; do :; done
  case "$last" in
    http*|ssh*|*.git) ;;
    *) mkdir -p "$last" ;;
  esac
fi
exit 0
""",
}


def step_run(workflow, job, step_name):
    data = yaml.safe_load((REPO / ".github/workflows" / workflow).read_text())
    for step in data["jobs"][job]["steps"]:
        if step.get("name") == step_name:
            return step["run"]
    raise SystemExit(f"step not found: {job} / {step_name}")


def run_block(name, script, workdir, extra_env=None):
    script = script.replace("${{ inputs.version }}", VERSION)
    script = script.replace("${{ github.repository_owner }}", "danil0ws")
    script = script.replace("${{ secrets.SCOOP_BUCKET_TOKEN }}", "x")
    script = script.replace("${{ steps.version.outputs.VERSION }}", VERSION)
    env = dict(os.environ)
    env["PATH"] = f"{STUB_DIR}:{env['PATH']}"
    env["GIT_LOG"] = str(STUB_DIR.parent / "git.log")
    if extra_env:
        env.update(extra_env)
    proc = subprocess.run(
        ["bash", "-euo", "pipefail", "-c", script],
        cwd=workdir,
        env=env,
        capture_output=True,
        text=True,
    )
    status = "ok" if proc.returncode == 0 else f"FAILED ({proc.returncode})"
    print(f"[{status}] {name}")
    if proc.returncode != 0:
        print(proc.stdout)
        print(proc.stderr, file=sys.stderr)
        failures.append(name)
    return proc


def check(name, condition, detail=""):
    if condition:
        print(f"  ✓ {name}")
    else:
        print(f"  ✗ {name} {detail}")
        failures.append(name)


def check_workflow_yaml():
    """Every workflow/action must be valid YAML with a jobs or runs shape."""
    for path in sorted((REPO / ".github").rglob("*.yml")):
        try:
            data = yaml.safe_load(path.read_text())
        except Exception as exc:  # noqa: BLE001 - report and continue
            check(f"{path.name} parses", False, str(exc))
            continue
        # workflows have `jobs`, composite actions have `runs`
        jobs = data.get("jobs") if isinstance(data, dict) else None
        runs = data.get("runs") if isinstance(data, dict) else None
        what = f"{len(jobs)} jobs" if jobs else ("composite action" if runs else "")
        check(f"{path.name} parses ({what})", bool(jobs or runs))


tmp = Path(tempfile.mkdtemp(prefix="klean-pkgcheck-"))
STUB_DIR = tmp / "stubs"
STUB_DIR.mkdir()
for tool, body in STUBS.items():
    p = STUB_DIR / tool
    p.write_text(body)
    p.chmod(0o755)
failures = []

check_workflow_yaml()

# ---------------------------------------------------------------- AUR -------
aur = tmp / "aur"
# the `git clone` step is a separate `run:` block; stand in for it here
(aur / "aur-klean").mkdir(parents=True, exist_ok=True)
run_block(
    "AUR: PKGBUILD + .SRCINFO",
    step_run("aur-update.yml", "update-aur", "Update PKGBUILD and .SRCINFO"),
    aur,
)
pkgbuild = (aur / "aur-klean" / "PKGBUILD").read_text()
srcinfo = (aur / "aur-klean" / ".SRCINFO").read_text()
print(pkgbuild)
check("PKGBUILD is valid bash", subprocess.run(["bash", "-n", "-c", pkgbuild]).returncode == 0)
check("pkgver is the release version", f"pkgver={DOT}" in pkgbuild)
check("sha256 filled in", "sha256sums=('1111" in pkgbuild, pkgbuild)
check("no unresolved ${VERSION}", "${VERSION}" not in pkgbuild)
check(".SRCINFO has tab-indented fields", "\tpkgver = " + DOT in srcinfo)
check(".SRCINFO pkgbase/pkgname", "pkgbase = klean" in srcinfo and "pkgname = klean" in srcinfo)

# -------------------------------------------------------------- Scoop -------
scoop = tmp / "scoop"
(scoop / "scoop-bucket" / "bucket").mkdir(parents=True)
run_block("Scoop: manifest", step_run("scoop-update.yml", "update-scoop", "Generate manifest"), scoop)
import json

manifest = json.loads((scoop / "scoop-bucket" / "bucket" / "klean.json").read_text())
check("manifest version", manifest["version"] == DOT)
check("asset url pinned to the tag", ASSET in manifest["url"], manifest["url"])
check("hash is sha256", len(manifest["hash"]) == 64)
check("checkver github", manifest["checkver"] == "github")
check("autoupdate keeps $version", "$version" in manifest["autoupdate"]["url"])

# ------------------------------------------------------------- winget -------
winget = tmp / "winget"
winget.mkdir()
run_block("winget: manifests", step_run("windows-packages.yml", "update-winget", "Prepare WinGet manifest"), winget)
manifests = sorted(winget.glob("winget-manifest/**/*.yaml"))
check("three manifests generated", len(manifests) == 3, str([m.name for m in manifests]))
docs = {p.name: yaml.safe_load(p.read_text()) for p in manifests}
installer = docs["klean.installer.yaml"]
check("version expanded (not literal ${})", installer["PackageVersion"] == DOT, installer["PackageVersion"])
check("zip installer declares a nested portable installer", installer["Installers"][0]["InstallerType"] == "zip" and installer["Installers"][0]["NestedInstallerType"] == "portable")
check("portable alias exposes `klean`", installer["Installers"][0]["NestedInstallerFiles"][0]["PortableCommandAlias"] == "klean")
check("installer url pinned", ASSET in installer["Installers"][0]["InstallerUrl"], installer["Installers"][0]["InstallerUrl"])
check("locale has no ${} left", "${" not in yaml.safe_dump(docs["klean.locale.en-US.yaml"]))
check("version manifest default locale", docs["klean.yaml"]["DefaultLocale"] == "en-US")

# ----------------------------------------------------------- Chocolatey -----
choco = tmp / "choco"
(choco / "choco-repo").mkdir(parents=True)
# the real step is PowerShell; emulate the two substitutions it performs and
# check the *template* we ship in examples/ keeps its placeholders.
install_ps1 = (REPO / "examples/package-managers/chocolatey/chocolateyinstall.ps1").read_text()
check("install script uses Install-ChocolateyZipPackage", "Install-ChocolateyZipPackage" in install_ps1)
check("install script is a placeholder template", "__URL__" in install_ps1 and "__SHA__" in install_ps1)
check("install script has no unsubstituted ${}", "${" not in install_ps1)

# ------------------------------------------------------------ Homebrew ------
brew = tmp / "brew"
(brew / "homebrew-klean" / "Formula").mkdir(parents=True)
(REPO / "LICENSE").read_text()  # sanity: exists for the cp step
run_block(
    "Homebrew: formula",
    step_run("homebrew-update.yml", "update-homebrew", "Update formula"),
    brew,
)
formula = (brew / "homebrew-klean" / "Formula" / "klean.rb").read_text()
print(formula)
check("formula is valid ruby", subprocess.run(["ruby", "-c", str(brew / "homebrew-klean" / "Formula" / "klean.rb")], capture_output=True).returncode == 0)
check("explicit version set", f'version "{DOT}"' in formula)
check("arm64 asset present", "aarch64-apple-darwin.tar.gz" in formula)
check("x86_64 asset present", "x86_64-apple-darwin.tar.gz" in formula)
check("no unresolved ${}", "${" not in formula)

print()
if failures:
    print(f"FAILURES ({len(failures)}): {failures}")
    sys.exit(1)
print("all package generation checks passed")
shutil.rmtree(tmp, ignore_errors=True)
