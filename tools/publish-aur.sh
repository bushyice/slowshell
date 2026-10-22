#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

target_pkg="${1:-all}"

packages=()
case "$target_pkg" in
  all)
    packages=("slowshell" "slowshell-bin")
    ;;
  slowshell|slowshell-bin)
    packages=("$target_pkg")
    ;;
  *)
    echo "Usage: $0 [slowshell|slowshell-bin|all]" >&2
    exit 1
    ;;
esac

echo "==> Testing SSH connection to AUR..."
if ! ssh -o BatchMode=yes -o ConnectTimeout=5 -T aur@aur.archlinux.org 2>&1 | grep -q "interactive shells"; then
  echo "note: ssh -T aur@aur.archlinux.org requires a configured SSH key registered with your AUR account."
  echo "      Make sure your SSH key is added to ssh-agent or ~/.ssh/config for host aur.archlinux.org."
fi

for pkg in "${packages[@]}"; do
  pkg_dir="$root/aur/$pkg"
  if [ ! -d "$pkg_dir" ]; then
    echo "error: directory $pkg_dir not found" >&2
    exit 1
  fi

  echo "==> Regenerating .SRCINFO for $pkg..."
  (cd "$pkg_dir" && makepkg --printsrcinfo > .SRCINFO)

  echo "==> Preparing $pkg for AUR..."
  tmp="$(mktemp -d)"

  aur_repo="ssh://aur@aur.archlinux.org/${pkg}.git"
  if git clone "$aur_repo" "$tmp/$pkg" 2>/dev/null; then
    echo "    Cloned existing AUR repo for $pkg"
  else
    echo "    Initializing new AUR repo for $pkg"
    mkdir -p "$tmp/$pkg"
    (
      cd "$tmp/$pkg"
      git init -b master
      git remote add origin "$aur_repo"
    )
  fi

  cp "$pkg_dir/PKGBUILD" "$pkg_dir/.SRCINFO" "$tmp/$pkg/"
  (
    cd "$tmp/$pkg"
    git add PKGBUILD .SRCINFO
    if git diff --cached --quiet; then
      echo "    No changes to commit for $pkg in AUR"
    else
      pkgver="$(sed -n 's/^pkgver = //p' .SRCINFO | head -n1)"
      pkgrel="$(sed -n 's/^pkgrel = //p' .SRCINFO | head -n1)"
      git commit -m "Update to ${pkgver}-${pkgrel}"
      echo "    Pushing $pkg (${pkgver}-${pkgrel}) to AUR..."
      git push origin master
      echo "    ✓ Successfully published $pkg to AUR"
    fi
  )
  rm -rf "$tmp"
done

echo "==> AUR processing complete!"
