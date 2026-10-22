#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

command -v gh >/dev/null || {
  echo "error: gh is not installed" >&2
  exit 1
}

remote=""
for candidate in github origin; do
  url="$(git remote get-url "$candidate" 2>/dev/null || true)"
  case "$url" in
    *github.com*) remote="$candidate"; break ;;
  esac
done
if [ -z "$remote" ]; then
  echo "error: no GitHub remote (looked for 'github' and 'origin')" >&2
  exit 1
fi

repo="$(git remote get-url "$remote" | sed -E 's#(git@|https://)github\.com[:/]##; s#\.git$##')"

version="$(cargo pkgid -p slowshell)"
version="${version##*[#@]}"
tag="${1:-v${version}}"

if [ -n "$(git status --porcelain)" ]; then
  echo "warning: working tree is dirty; $tag will point at $(git rev-parse --short HEAD)" >&2
fi

echo "==> building slowshell $version"
if command -v cargo-zigbuild >/dev/null 2>&1; then
  echo "    using cargo-zigbuild (target: x86_64-unknown-linux-gnu.2.39)"
  cargo zigbuild --target x86_64-unknown-linux-gnu.2.39 --release --locked -p slowshell --bin slowshell
  bin="target/x86_64-unknown-linux-gnu/release/slowshell"
else
  echo "    warning: cargo-zigbuild not found, falling back to standard cargo build"
  cargo build --release --locked -p slowshell --bin slowshell
  bin="target/release/slowshell"
fi

echo "==> packaging"
pkg="slowshell-${version}"
dist="$root/dist"
rm -rf "$dist/$pkg"
mkdir -p "$dist/$pkg"
cp "$bin" "$dist/$pkg/slowshell"
chmod 0755 "$dist/$pkg/slowshell"
# for extra in themes example.kdl; do
#   [ -e "$extra" ] && cp -r "$extra" "$dist/$pkg/"
# done
tar -czf "$dist/$pkg.tar.gz" -C "$dist" "$pkg"
(cd "$dist" && sha256sum "$pkg.tar.gz" > "$pkg.tar.gz.sha256")
cp "$dist/$pkg.tar.gz" "$dist/${pkg}-glibc2.39.tar.gz"
(cd "$dist" && sha256sum "${pkg}-glibc2.39.tar.gz" > "${pkg}-glibc2.39.tar.gz.sha256")
echo "    dist/$pkg.tar.gz"
echo "    dist/${pkg}-glibc2.39.tar.gz"

echo "==> tagging $tag"
if ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  git tag -a "$tag" -m "$tag"
fi
git push "$remote" "refs/tags/$tag"

echo "==> publishing to GitHub ($repo)"
assets=(
  "$dist/$pkg.tar.gz"
  "$dist/$pkg.tar.gz.sha256"
  "$dist/${pkg}-glibc2.39.tar.gz"
  "$dist/${pkg}-glibc2.39.tar.gz.sha256"
)
if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  gh release upload "$tag" "${assets[@]}" --repo "$repo" --clobber
else
  gh release create "$tag" "${assets[@]}" \
    --repo "$repo" --title "$tag" --generate-notes
fi

gh release view "$tag" --repo "$repo" --json url -q .url
