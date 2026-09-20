#!/bin/sh
# klean installer for Linux/macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/danil0ws/klean/main/scripts/install.sh | sh
#   sh install.sh --version v1.0.0 --prefix /usr/local/bin
#
# Downloads the release tarball for this OS/arch, verifies it against the
# release SHA256SUMS and installs the binary. No package manager required.
set -eu

REPO="${KLEAN_REPO:-danil0ws/klean}"
VERSION="${KLEAN_VERSION:-latest}"
PREFIX="${KLEAN_PREFIX:-$HOME/.local/bin}"
DRY_RUN=""

usage() {
    cat <<'EOF'
uso: install.sh [--version vX.Y.Z] [--prefix DIR] [--dry-run]

  --version  versão a instalar (padrão: última release)
  --prefix   diretório de destino (padrão: ~/.local/bin)
  --dry-run  só mostra o que faria
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:?--version precisa de um valor}"; shift 2 ;;
        --prefix) PREFIX="${2:?--prefix precisa de um valor}"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "argumento desconhecido: $1" >&2; usage; exit 1 ;;
    esac
done

case "$(uname -s)" in
    Linux)
        case "$(uname -m)" in
            x86_64|amd64) TARGET=x86_64-unknown-linux-musl ;;
            aarch64|arm64) TARGET=aarch64-unknown-linux-musl ;;
            *) echo "arquitetura não suportada: $(uname -m)" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$(uname -m)" in
            x86_64) TARGET=x86_64-apple-darwin ;;
            arm64) TARGET=aarch64-apple-darwin ;;
            *) echo "arquitetura não suportada: $(uname -m)" >&2; exit 1 ;;
        esac
        ;;
    *)
        echo "no Windows use: irm https://raw.githubusercontent.com/${REPO}/main/scripts/install.ps1 | iex" >&2
        exit 1
        ;;
esac

if [ "$VERSION" = "latest" ]; then
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" |
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
    if [ -z "$VERSION" ]; then
        echo "não consegui descobrir a última release (rede ou rate limit)" >&2
        exit 1
    fi
fi

ASSET="klean-${VERSION}-${TARGET}.tar.gz"
BASE="https://github.com/${REPO}/releases/download/${VERSION}"

echo "klean ${VERSION} para ${TARGET}"
echo "  de:     ${BASE}/${ASSET}"
echo "  para:   ${PREFIX}/klean"

if [ -n "$DRY_RUN" ]; then
    echo "(dry-run) nada foi baixado nem instalado"
    exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fL --retry 3 --retry-delay 1 -o "$TMP/$ASSET" "${BASE}/${ASSET}"

if curl -fsSL -o "$TMP/SHA256SUMS" "${BASE}/SHA256SUMS"; then
    if grep -q " ${ASSET}\$" "$TMP/SHA256SUMS"; then
        grep " ${ASSET}\$" "$TMP/SHA256SUMS" > "$TMP/sums"
        (cd "$TMP" && if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c sums
        else
            shasum -a 256 -c sums
        fi)
    else
        echo "aviso: ${ASSET} não está no SHA256SUMS" >&2
    fi
else
    echo "aviso: SHA256SUMS indisponível, seguindo sem verificar" >&2
fi

tar xzf "$TMP/$ASSET" -C "$TMP"
mkdir -p "$PREFIX"
install -m 0755 "$TMP/klean" "$PREFIX/klean"

echo "✓ instalado em ${PREFIX}/klean"
case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *) echo "  adicione ao PATH:  export PATH=\"$PREFIX:\$PATH\"" ;;
esac
"$PREFIX/klean" --version
