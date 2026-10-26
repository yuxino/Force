#!/usr/bin/env bash
set -euo pipefail

workspace="$(cd "$(dirname "$0")/.." && pwd)"
cd "$workspace"

if [[ "$(uname -s)" != Darwin ]]; then
  echo 'macOS is required to create and sign an app bundle.' >&2
  exit 1
fi

profile=debug
production=false
ad_hoc=false
for argument in "$@"; do
  case "$argument" in
    --release) profile=release ;;
    --production) production=true ;;
    --ad-hoc) ad_hoc=true ;;
    -h|--help)
      echo 'Usage: scripts/build-macos.sh [--release] [--production] [--ad-hoc]'
      echo 'Default: signed build/Force Dev.app, isolated development identity.'
      echo 'FORCE_SIGN_IDENTITY selects an existing code-signing identity by hash or name.'
      exit 0
      ;;
    *) echo "Unknown argument: $argument" >&2; exit 1 ;;
  esac
done

if [[ "$ad_hoc" == true ]]; then
  identity=-
else
  identities="$(security find-identity -v -p codesigning)"
  requested_identity="${FORCE_SIGN_IDENTITY:-}"
  if [[ -n "$requested_identity" ]]; then
    identity="$(printf '%s\n' "$identities" | awk -v wanted="$requested_identity" '
      /[0-9A-Fa-f]{40}/ {
        name = $0; sub(/^[^"]*"/, "", name); sub(/".*$/, "", name)
        if ($2 == wanted || name == wanted) print $2
      }')"
  else
    identity="$(printf '%s\n' "$identities" | awk '/"Apple Development:/ { print $2 }')"
  fi
  if [[ -z "$identity" ]] || [[ "$identity" == *$'\n'* ]]; then
    echo 'Select one valid private-key-backed identity with FORCE_SIGN_IDENTITY.' >&2
    echo 'The default requires exactly one Apple Development identity.' >&2
    exit 1
  fi
fi

bundle_name='Force Dev'
bundle_id=app.yuxino.force.dev
if [[ "$production" == true ]]; then
  bundle_name=Force
  bundle_id=app.yuxino.force
fi
bundle="$workspace/build/$bundle_name.app"
if [[ -f "$bundle/Contents/MacOS/force" ]] && lsof -t "$bundle/Contents/MacOS/force" >/dev/null 2>&1; then
  echo "Quit the running $bundle_name app before rebuilding its bundle." >&2
  exit 1
fi

export MACOSX_DEPLOYMENT_TARGET=15.0
build_arguments=(build --bin force --locked)
if [[ "$profile" == release ]]; then build_arguments+=(--release); fi
cargo "${build_arguments[@]}"

target_directory="${CARGO_TARGET_DIR:-$workspace/target}"
if [[ "$target_directory" != /* ]]; then target_directory="$workspace/$target_directory"; fi
if [[ -n "${CARGO_BUILD_TARGET:-}" ]]; then target_directory="$target_directory/$CARGO_BUILD_TARGET"; fi
binary="$target_directory/$profile/force"
if [[ ! -x "$binary" ]]; then
  echo 'The native executable was not found in the Cargo target directory.' >&2
  exit 1
fi

mkdir -p "$workspace/build"
staging="$(mktemp -d "$workspace/build/.force-bundle.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
staged_bundle="$staging/$bundle_name.app"
mkdir -p "$staged_bundle/Contents/MacOS" "$staged_bundle/Contents/Resources"
cp "$binary" "$staged_bundle/Contents/MacOS/force"
cp assets/Force.icns "$staged_bundle/Contents/Resources/Force.icns"
version="$(awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml)"
cat > "$staged_bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$bundle_name</string>
  <key>CFBundleDisplayName</key><string>$bundle_name</string>
  <key>CFBundleIdentifier</key><string>$bundle_id</string>
  <key>CFBundleExecutable</key><string>force</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleIconFile</key><string>Force</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST
plutil -lint "$staged_bundle/Contents/Info.plist" >/dev/null
codesign --force --options runtime --timestamp=none --sign "$identity" "$staged_bundle"
codesign --verify --deep --strict "$staged_bundle"
if [[ -f "$bundle/Contents/MacOS/force" ]] && lsof -t "$bundle/Contents/MacOS/force" >/dev/null 2>&1; then
  echo "The $bundle_name app started during the build. Quit it before replacing its bundle." >&2
  exit 1
fi
if [[ -e "$bundle" ]]; then rm -rf "$bundle"; fi
mv "$staged_bundle" "$bundle"
printf 'Signed bundle: build/%s.app\n' "$bundle_name"
echo 'This is a local signed build; it has not been notarized or published.'
