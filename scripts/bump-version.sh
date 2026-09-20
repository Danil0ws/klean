#!/usr/bin/env bash
# Bump klean's version in every file that carries it, or verify they agree.
#
#   scripts/bump-version.sh 1.2.0     # rewrite every copy
#   scripts/bump-version.sh --check   # CI: fail when a copy drifted
#
# Cargo.toml is the source of truth: the script reads the old version from it
# and replaces that exact string everywhere else (manifests, docs, installers,
# CI workflow defaults, Cargo.lock). Carriers are the packaged templates that
# ship to package managers; `--check` asserts those always match Cargo.toml.
set -euo pipefail

# BSD sed (macOS) dies on non-UTF8 bytes *with* a UTF-8 locale; run byte-oriented.
export LC_ALL=C

cd "$(dirname "$0")/.."

CARRIERS=(
  examples/package-managers/scoop/klean.json
  examples/package-managers/aur/PKGBUILD
  examples/package-managers/homebrew/klean.rb
  examples/package-managers/chocolatey/klean.nuspec
)

crate_version() { grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2; }

if [[ "${1:-}" == "--check" ]]; then
  want=$(crate_version)
  drifts=0
  for file in "${CARRIERS[@]}"; do
    if ! grep -q -- "$want" "$file"; then
      echo "✗ $file não menciona $want ($(grep -oiE '[0-9]+\.[0-9]+\.[0-9]+' "$file" | head -1))"
      drifts=$((drifts + 1))
    fi
  done
  if [[ $drifts -gt 0 ]]; then
    echo "→ rode: scripts/bump-version.sh $want"
    exit 1
  fi
  echo "✓ versão $want em Cargo.toml e nos ${#CARRIERS[@]} manifestos"
  exit 0
fi

new="${1:-}"
if [[ ! $new =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "uso: $0 <x.y.z>   |   $0 --check" >&2
  exit 2
fi

old=$(crate_version)
if [[ $old == "$new" ]]; then
  echo "já está em $new"
  exit 0
fi

# Every file still naming the current version, build output and vendored trees
# excluded. -i.bak works on both BSD and GNU sed; the backup is thrown away.
files=()
# BSD grep (macOS) ignores --exclude-dir, so drop build/SCM noise here instead.
while IFS= read -r file; do
  case "$file" in
    ./.git/*|./target/*|./node_modules/*) continue ;;
  esac
  files+=("$file")
done < <(grep -rl -- "$old" . 2>/dev/null || true)

for file in "${files[@]}"; do
  sed -i.bak "s/${old//./\\.}/$new/g" "$file"
  rm -f "$file.bak"
done

# Cargo.lock mentions the version too; let cargo confirm the tree still solves.
cargo metadata --no-deps --format-version 1 >/dev/null

echo "✓ $old → $new em ${#files[@]} arquivo(s):"
printf '  %s\n' "${files[@]}"
echo "→ agora: scripts/bump-version.sh --check && cargo test"
