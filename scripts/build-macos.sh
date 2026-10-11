#!/bin/zsh
set -euo pipefail
cd "${0:A:h}/.."
: "${FORCE_SIGN_IDENTITY:?Set FORCE_SIGN_IDENTITY to an existing private-key-backed signing identity}"
if ! security find-identity -v -p codesigning | grep -Fq "$FORCE_SIGN_IDENTITY"; then
  print -u2 'Signing identity is unavailable; refusing to produce a launchable app.'
  exit 1
fi
export CGO_CFLAGS="-mmacosx-version-min=13.0"
export CGO_LDFLAGS="-mmacosx-version-min=13.0"
wails build -platform darwin/arm64 -trimpath -nocolour
codesign --force --options runtime --timestamp=none --sign "$FORCE_SIGN_IDENTITY" build/bin/Force.app
codesign --verify --deep --strict build/bin/Force.app
print 'Signed: build/bin/Force.app'
