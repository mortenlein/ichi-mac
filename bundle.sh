#!/usr/bin/env bash
#
# Builds Ichi.app — a background agent bundle.
#
# The bundle is not cosmetic. macOS ties Accessibility permission to a code
# signature, so a bare `target/release/ichi` run from a terminal gets the
# permission attributed to the *terminal*, not to Ichi. Signing the bundle
# gives Ichi a stable identity that survives rebuilds, which is what keeps the
# permission granted instead of needing re-approval every time.

set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="Ichi"
BUNDLE="target/${APP_NAME}.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

echo "==> Building release binary"
cargo build --release

echo "==> Assembling ${BUNDLE}"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
cp target/release/ichi "$BUNDLE/Contents/MacOS/ichi"

cat > "$BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>       <string>${APP_NAME}</string>
    <key>CFBundleExecutable</key>        <string>ichi</string>
    <key>CFBundleIdentifier</key>        <string>com.mortenlein.ichi</string>
    <key>CFBundleVersion</key>           <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleIconFile</key>          <string>ichi</string>
    <key>LSMinimumSystemVersion</key>    <string>11.0</string>

    <!-- Stealth Mode: no Dock icon, no menu bar, never steals focus. -->
    <key>LSUIElement</key>               <true/>
</dict>
</plist>
PLIST

if [[ -f icon.png ]]; then
    echo "==> Generating icon"
    ICONSET="$(mktemp -d)/ichi.iconset"
    mkdir -p "$ICONSET"
    for size in 16 32 128 256 512; do
        sips -z $size $size icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null 2>&1
        sips -z $((size * 2)) $((size * 2)) icon.png \
             --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null 2>&1
    done
    iconutil -c icns "$ICONSET" -o "$BUNDLE/Contents/Resources/ichi.icns"
    rm -rf "$(dirname "$ICONSET")"
fi

# Ad-hoc signature. Replace "-" with a Developer ID to distribute this to
# anyone else — an ad-hoc signature is only trusted on the machine that made it.
echo "==> Signing (ad-hoc)"
codesign --force --deep --sign - "$BUNDLE"

echo
echo "Built ${BUNDLE}"
echo
echo "Next:"
echo "  1. mv ${BUNDLE} /Applications/"
echo "  2. open /Applications/${APP_NAME}.app        # prompts for Accessibility"
echo "  3. Grant it in System Settings > Privacy & Security > Accessibility"
echo "  4. Relaunch — macOS only applies the grant to a fresh process"
