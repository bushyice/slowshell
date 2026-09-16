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
cargo build --release --locked -p slowshell --bin slowshell

echo "==> packaging"
pkg="slowshell-${version}"
dist="$root/dist"
rm -rf "$dist/$pkg"
mkdir -p "$dist/$pkg"
cp target/release/slowshell "$dist/$pkg/slowshell"
# for extra in themes example.kdl; do
#   [ -e "$extra" ] && cp -r "$extra" "$dist/$pkg/"
# done
tar -czf "$dist/$pkg.tar.gz" -C "$dist" "$pkg"
sha256sum "$dist/$pkg.tar.gz" > "$dist/$pkg.tar.gz.sha256"
echo "    dist/$pkg.tar.gz"

echo "==> tagging $tag"
if ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  git tag -a "$tag" -m "$tag"
fi
git push "$remote" "refs/tags/$tag"

echo "==> publishing to GitHub ($repo)"
assets=("$dist/$pkg.tar.gz" "$dist/$pkg.tar.gz.sha256")
if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  gh release upload "$tag" "${assets[@]}" --repo "$repo" --clobber
else
  gh release create "$tag" "${assets[@]}" \
    --repo "$repo" --title "$tag" --generate-notes
fi

gh release view "$tag" --repo "$repo" --json url -q .url
