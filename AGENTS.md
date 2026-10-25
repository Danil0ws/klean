# AGENTS.md

Klean — CLI em Rust (gerenciador de configurações/ambiente de dev).

<!-- omh:agent-instructions:begin -->

## Stack / arquitetura

- CLI Rust com Cargo; código em `src/`, testes em `tests/`, scripts de release/embalagem em `scripts/` e workflows CI em `.github/workflows/`.
- `Cargo.toml` é a fonte única da versão — bump de versão passa por `scripts/bump-version.sh` (não edite versão à mão em outros arquivos).

## Mapa de pastas

- `src/` — núcleo do CLI (comandos, runtime).
- `tests/` — testes de integração `cargo`.
- `scripts/` — `bump-version.sh`, `check-packages.py` e helpers de release.
- `.github/workflows/` — CI (fmt/clippy/test gates).

## Regras da casa

- `cargo clippy` roda com `-D warnings` na CI: warning é erro.
- Formatação obrigatória: `cargo fmt` antes de commitar.
- Versão: só via `scripts/bump-version.sh`; `Cargo.toml` manda.
- `check-packages.py` exige PyYAML — rodar com `uv run --with pyyaml` (sem ela: `ModuleNotFoundError: yaml`).

## Comandos

- `cargo fmt --check` — verificado (exit 0, 2026-10-04).
- `cargo clippy -- -D warnings` — verificado (exit 0, 2026-10-04).
- `cargo test --all-targets` — verificado (exit 0, 2026-10-04).
- `scripts/bump-version.sh --check` — verificado (exit 0, 2026-10-04).
- `uv run --with pyyaml scripts/check-packages.py` — verificado (exit 0, 2026-10-04).
- `cargo build --release --locked` — não verificado (rodar antes de release).

## Armadilhas conhecidas

- CI falha em qualquer warning do clippy ou desvio de fmt.
- `check-packages.py` quebra sem PyYAML no ambiente.

<!-- omh:agent-instructions:end -->