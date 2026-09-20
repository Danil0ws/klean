# Committing and pushing on `main`

Everything goes straight to `main` in this repo: no develop branch, no PR
required for your own work. `ci.yml` runs on every push, so the checks below are
the thing that protects the branch — run them before pushing, not after.

[CONTRIBUTING.md](CONTRIBUTING.md) covers message conventions and PRs; this file
is the exact routine.

## TL;DR

```bash
# 1. see what is there
git status --short
git diff --cached --stat        # what is already staged

# 2. run what CI runs (see "Checks" below)
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets

# 3. commit and push
git commit -m "feat(scanner): report one directory per artifact"
git push origin main
```

## Rules for this branch

- Commit **directly on `main`** and push. Nothing is published by a push: only a
  tag (`v*`) starts `build-release.yml` and the package workflows.
- **Conventional Commits**: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`,
  `ci`, `perf`. Optional scope in parentheses: `fix(scanner):`, `ci(homebrew):`.
  Breaking change: `feat(scanner)!:` plus a `BREAKING CHANGE:` paragraph.
- One logical change per commit. Reviewers read `git log --oneline`, not diffs
  of five unrelated things.
- Never `git push --force` (or `--force-with-lease`) on `main`; if a pushed
  commit is wrong, `git revert` it.
- Never commit `.env*`, tokens, SSH keys or anything with a credential.
- Never `git commit --no-verify`: the hooks and CI exist for a reason.
- **Some docs are intentionally not in git.** `.gitignore` excludes
  `CI_CD.md`, `CI_CD_SUMMARY.md`, `CI_CD_ARCHITECTURE.md`,
  `CI_CD_TROUBLESHOOTING.md`, `SETUP_CI_CD.md`, `DOCUMENTATION_INDEX.md`,
  `RELEASE.md` and `PACKAGE_MANAGERS.md`. They stay local; `git add -A` will not
  pick them up and `git status` will not show them. `git status --ignored`
  shows what is being skipped.

## Checks (what `ci.yml` will run)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets                        # 30 tests
cargo build --release --locked
uv run --with pyyaml python scripts/check-packages.py   # package templates + workflow YAML
```

Red locally = red in CI = red release. Fix before pushing.

## Committing in logical steps

`git add -A` is fine when the tree is clean of junk, but a pile of unrelated
changes belongs in separate commits. Unstage everything and rebuild:

```bash
git restore --staged .          # keep the working tree, drop the index
```

Then one commit per group (`git add <paths>` and repeat):

| # | Commit | Paths |
|---|--------|-------|
| 1 | `fix(scanner): one directory per artifact, project-scoped deletion` | `src/scanner.rs src/ignore.rs src/patterns.rs src/config.rs src/cleaner.rs src/lib.rs tests/scanner_test.rs tests/integration_test.rs examples/klean.toml` |
| 2 | `feat(tui): column table, scrolling and terminal restore` | `src/tui/mod.rs src/tui/interactive.rs` |
| 3 | `feat: plugins, stats, JSON output, web UI, watch mode and CI gate` | `src/plugins.rs src/history.rs src/web.rs src/watch.rs src/cli.rs src/main.rs tests/web_test.rs Cargo.toml Cargo.lock` |
| 4 | `ci: reusable package workflows, working generators, ci.yml and installers` | `.github/ scripts/ examples/package-managers/ examples/ci/` |
| 5 | `docs: README, INSTALLATION and CONTRIBUTING for the new behaviour` | `README.md INSTALLATION.md CONTRIBUTING.md .gitignore` |

`src/main.rs` touches both the scanner wiring (1) and the new modes (3); pick one
commit for it and mention the rest in the body. That is cheaper than splitting
hunks.

One commit instead, if you prefer a single landing:

```bash
git add -A
git commit -m "feat: non-recursive scanner, project rules, automation and packaging"
```

### Message shape

```
fix(scanner): stop losing directories to walkdir's skip_current_dir

skip_current_dir() is a pop() of the directory stack, so calling it once per
matching pattern dropped an ancestor and half the tree was never scanned.
Matching now happens once per directory and traversal is explicit.

Tested with: cargo test --all-targets (tests/scanner_test.rs)
```

Subject ≤ 72 chars, imperative, no trailing period. Body explains **why**; the
diff already shows what.

## Pushing

```bash
git push origin main                      # normal push
git push -u origin main                   # only the first time on a fresh clone
```

What happens next:

- `ci.yml` runs (fmt, clippy, tests on Linux/macOS/Windows, release build,
  packaging templates). Watch it with `gh run watch` or on the Actions tab.
- `docker.yml` also runs on pushes to `main` and pushes new images.
- No package is published. That needs a tag.

## Releasing (after the push)

```bash
git tag -a v0.1.4 -m "klean v0.1.4"
git push origin v0.1.4
```

The tag starts `build-release.yml`, which builds the five targets, creates the
GitHub Release with `SHA256SUMS`, publishes to crates.io and then calls the
package workflows (Homebrew, Scoop, Chocolatey/winget/mise, deb/rpm/Arch, AUR).
Details: [RELEASE.md](RELEASE.md) and [CI_CD.md](CI_CD.md).

## Undoing

```bash
git restore --staged <path>            # unstage, keep the file changes
git restore <path>                     # throw away changes to a file (careful)
git commit --amend                     # fix the message / add a forgotten file (unpushed only)
git commit --amend --no-edit           # add staged files to the last commit

git reset --soft HEAD~1                # unpushed: undo the commit, keep it staged
git revert <sha>                       # pushed: safe undo, creates a new commit
```

`--amend` rewrites history: only do it while the commit is still local. After a
push, use `git revert`.

## Quick reference

```bash
git status --short                     # M = unstaged, A = added, ?? = untracked
git status --ignored --short           # shows local-only docs (!!)
git diff --cached                      # what the next commit contains
git log --oneline -10                  # history style to follow
git log -1 --stat                      # what the last commit touched
git remote -v && git branch -vv        # where main points
```
