#!/usr/bin/env bash
# curl -fsSL https://raw.githubusercontent.com/bushyice/slowshell/main/install.sh | sh

set -euo pipefail

REPO="bushyice/slowshell"
BIN="slowshell"
API="https://api.github.com/repos/${REPO}/releases"
DOWNLOAD="https://github.com/${REPO}/releases/download"

version=""
install_dir=""

usage() {
  cat <<'EOF'
slowshell installer

Options:
  --version <v>   release to install (default: latest); "0.0.1" or "v0.0.1"
  --path <dir>    directory to install the slowshell binary into
                  (default: ~/.local/bin, or /usr/local/bin when run as root)
  -h, --help      show this
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || { echo "error: --version needs a value" >&2; exit 1; }
      version="$2"; shift 2 ;;
    --version=*) version="${1#*=}"; shift ;;
    --path)
      [ "$#" -ge 2 ] || { echo "error: --path needs a value" >&2; exit 1; }
      install_dir="$2"; shift 2 ;;
    --path=*) install_dir="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if command -v curl >/dev/null 2>&1; then
  download() { curl -fsSL "$1" -o "$2"; }
  download_stdout() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget -qO "$2" "$1"; }
  download_stdout() { wget -qO- "$1"; }
else
  echo "error: curl or wget is required" >&2
  exit 1
fi

[ "$(uname -s)" = "Linux" ] || {
  echo "error: slowshell only runs on Linux" >&2
  exit 1
}
if [ "$(uname -m)" != "x86_64" ]; then
  echo "warning: releases are currently built for x86_64; $(uname -m) may not work" >&2
fi

if [ -z "$version" ]; then
  echo "==> finding the latest release"
  api_json="$(download_stdout "$API/latest")" || {
    echo "error: could not reach the GitHub API; pass --version <v>" >&2
    exit 1
  }
  tag="$(printf '%s' "$api_json" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')"
  if [ -z "$tag" ]; then
    echo "error: could not determine the latest release; pass --version <v>" >&2
    exit 1
  fi
else
  case "$version" in
    v*) tag="$version" ;;
    *) tag="v$version" ;;
  esac
fi

version="${tag#v}"
asset="${BIN}-${version}.tar.gz"
url="${DOWNLOAD}/${tag}/${asset}"

if [ -z "$install_dir" ]; then
  if [ "$(id -u)" -eq 0 ]; then
    install_dir="/usr/local/bin"
  else
    install_dir="${HOME}/.local/bin"
  fi
fi
case "$install_dir" in
  "~") install_dir="$HOME" ;;
  "~/"*) install_dir="${HOME}/${install_dir#\~/}" ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "==> downloading ${asset}"
download "$url" "$tmp/$asset" || {
  echo "error: failed to download $url" >&2
  exit 1
}

if download "${url}.sha256" "$tmp/$asset.sha256" 2>/dev/null; then
  expected="$(awk 'NR==1 { print $1 }' "$tmp/$asset.sha256")"
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$tmp/$asset" | awk '{ print $1 }')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$tmp/$asset" | awk '{ print $1 }')"
  else
    actual=""
  fi
  if [ -n "$expected" ] && [ -n "$actual" ] && [ "$expected" != "$actual" ]; then
    echo "error: checksum mismatch for $asset" >&2
    exit 1
  fi
fi

echo "==> extracting"
tar -xzf "$tmp/$asset" -C "$tmp"
src="$(find "$tmp" -type f -name "$BIN" -print -quit)"
[ -n "$src" ] || {
  echo "error: $BIN not found in $asset" >&2
  exit 1
}

echo "==> installing to ${install_dir}/${BIN}"
if mkdir -p "$install_dir" 2>/dev/null && [ -w "$install_dir" ]; then
  cp "$src" "$install_dir/$BIN"
  chmod 0755 "$install_dir/$BIN"
else
  sudo mkdir -p "$install_dir"
  sudo cp "$src" "$install_dir/$BIN"
  sudo chmod 0755 "$install_dir/$BIN"
fi

echo "==> installed ${install_dir}/${BIN}"
case ":${PATH}:" in
  *":${install_dir}:"*) ;;
  *) echo "note: ${install_dir} is not on your PATH" ;;
esac
