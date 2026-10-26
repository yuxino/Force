#!/usr/bin/env bash
set -euo pipefail
workspace="$(cd "$(dirname "$0")/.." && pwd)"
cd "$workspace"
version="$(awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml)"
architecture="${CARGO_BUILD_TARGET:-$(rustc -vV | awk '/^host:/ {print $2}')}"
architecture="${architecture%%-*}"
case "$architecture" in aarch64|x86_64) ;; *) echo 'Unsupported macOS architecture' >&2; exit 1 ;; esac
bundle="$workspace/build/Force.app"
[[ -x "$bundle/Contents/MacOS/force" ]]
native_architecture="$(uname -m)"
if [[ "${architecture/aarch64/arm64}" == "$native_architecture" ]]; then
  [[ "$("$bundle/Contents/MacOS/force" --version)" == "Force $version" ]]
fi
[[ "$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$bundle/Contents/Info.plist")" == "$version" ]]
lipo "$bundle/Contents/MacOS/force" -verify_arch "${architecture/aarch64/arm64}"
codesign --verify --deep --strict "$bundle"
output="$workspace/build/release"
mkdir -p "$output"
staging="$(mktemp -d "$workspace/build/.force-dmg.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
ditto "$bundle" "$staging/Force.app"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -volname Force -srcfolder "$staging" -ov -format UDZO \
  "$output/Force-v$version-macos-$architecture.dmg"
echo "Created Force-v$version-macos-$architecture.dmg"
