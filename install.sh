#!/usr/bin/env sh
# Installs the latest `clock` release for Linux or macOS.
#   curl -fsSL https://raw.githubusercontent.com/HainanZhao/tictock/main/install.sh | sh
set -eu

REPO="HainanZhao/tictock"
INSTALL_DIR="${CLOCK_INSTALL_DIR:-/usr/local/bin}"

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

url="https://github.com/${REPO}/releases/download/${tag}/clock-${target}.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading clock ${tag} for ${target}..."
curl -fsSL "$url" -o "$tmp/clock.tar.gz"
tar -xzf "$tmp/clock.tar.gz" -C "$tmp"

if [ -w "$INSTALL_DIR" ]; then
  mv "$tmp/clock" "$INSTALL_DIR/clock"
else
  echo "Need sudo to write to $INSTALL_DIR"
  sudo mv "$tmp/clock" "$INSTALL_DIR/clock"
fi
chmod +x "$INSTALL_DIR/clock"

echo "Installed to ${INSTALL_DIR}/clock"
echo "Run 'clock' to start, or 'clock --help' for options."
