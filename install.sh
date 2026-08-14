#!/bin/sh
set -eu

repo="k8s-1/bashls"
install_dir="${BASHLS_INSTALL_DIR:-$HOME/.local/bin}"

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Linux) os_part="unknown-linux-musl" ;;
  Darwin) os_part="apple-darwin" ;;
  *)
    echo "error: unsupported OS: $os" >&2
    exit 1
    ;;
esac

case "$arch" in
  x86_64|amd64) arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *)
    echo "error: unsupported architecture: $arch" >&2
    exit 1
    ;;
esac

target="${arch_part}-${os_part}"
url="https://github.com/${repo}/releases/latest/download/bashls-${target}.tar.gz"

mkdir -p "$install_dir"
echo "Installing bashls (${target}) to ${install_dir}..." >&2

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

curl -fsSL "$url" | tar xz -C "$tmp_dir"
mv "$tmp_dir/bashls" "$install_dir/bashls"
chmod +x "$install_dir/bashls"

echo "Installed $("$install_dir/bashls" --version) to ${install_dir}/bashls" >&2

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) echo "warning: ${install_dir} is not on your \$PATH" >&2 ;;
esac
