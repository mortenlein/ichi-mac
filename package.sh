#!/usr/bin/env bash
#
# Packages an already-built target/Ichi.app into release artifacts:
#   target/Ichi-<version>-macos.zip   direct download
#   target/Ichi-<version>.dmg         drag-to-Applications disk image
#
# Split out from bundle.sh because notarization has to happen between building
# the .app and packaging it — the release workflow signs, notarizes, staples,
# and only then calls this.
#
# Usage:  ./bundle.sh --universal && ./package.sh

set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="Ichi"
BUNDLE="target/${APP_NAME}.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

if [[ ! -d "$BUNDLE" ]]; then
    echo "error: $BUNDLE not found — run ./bundle.sh first" >&2
    exit 1
fi

ZIP="target/${APP_NAME}-${VERSION}-macos.zip"
DMG="target/${APP_NAME}-${VERSION}.dmg"

echo "==> Creating ${ZIP}"
# ditto rather than zip: it preserves the code signature and symlinks, which a
# plain `zip` silently corrupts, producing an app Gatekeeper then rejects.
rm -f "$ZIP"
ditto -c -k --keepParent "$BUNDLE" "$ZIP"

echo "==> Creating ${DMG}"
STAGING="$(mktemp -d)"
cp -R "$BUNDLE" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
rm -f "$DMG"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING" \
    -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGING"

echo
ls -lh "$ZIP" "$DMG" | awk '{print "    "$9"  "$5}'
