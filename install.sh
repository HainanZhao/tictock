#!/usr/bin/env sh
# Installs the latest `tictock` release for Linux or macOS.
#   curl -fsSL https://raw.githubusercontent.com/HainanZhao/tictock/main/install.sh | sh
set -eu

REPO="HainanZhao/tictock"
INSTALL_DIR="${TICTOCK_INSTALL_DIR:-${CLOCK_INSTALL_DIR:-/usr/local/bin}}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux) platform="unknown-linux-gnu" ;;
  Darwin) platform="apple-darwin" ;;
  *) echo "error: unsupported OS: $os (see README.md for manual install)" >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64) cpu="x86_64" ;;
  arm64|aarch64) cpu="aarch64" ;;
  *) echo "error: unsupported architecture: $arch" >&2; exit 1 ;;
esac

target="${cpu}-${platform}"
tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep -m1 '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')"
if [ -z "$tag" ]; then
  echo "error: could not determine the latest release" >&2
  exit 1
fi

url="https://github.com/${REPO}/releases/download/${tag}/tictock-${target}.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading tictock ${tag} for ${target}..."
curl -fsSL "$url" -o "$tmp/tictock.tar.gz"
tar -xzf "$tmp/tictock.tar.gz" -C "$tmp"

if [ -w "$INSTALL_DIR" ]; then
  mv "$tmp/tictock" "$INSTALL_DIR/tictock"
else
  echo "Need sudo to write to $INSTALL_DIR"
  sudo mv "$tmp/tictock" "$INSTALL_DIR/tictock"
fi
chmod +x "$INSTALL_DIR/tictock"

echo "Installed to ${INSTALL_DIR}/tictock"
echo "Run 'tictock' to start, or 'tictock --help' for options."
