#!/usr/bin/env bash
#
# Builds Ichi.app — a background agent bundle.
#
# The bundle is not cosmetic. macOS ties Accessibility permission to a code
# signature, so a bare `target/release/ichi` run from a terminal gets the
# permission attributed to the *terminal*, not to Ichi. Signing the bundle
# gives Ichi a stable identity that survives rebuilds, which is what keeps the
# permission granted instead of needing re-approval every time.
#
# Usage:
#   ./bundle.sh                  native arch, ad-hoc signed
#   ./bundle.sh --universal      arm64 + x86_64 universal binary
#   ./bundle.sh --universal --dmg  also produce a drag-to-Applications disk image
#   ./bundle.sh --install        install to /Applications and relaunch
#
# Set SIGN_IDENTITY to a Developer ID to sign for distribution:
#   SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./bundle.sh

set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="Ichi"
BUNDLE="target/${APP_NAME}.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

# Ad-hoc ("-") unless a real identity is supplied. An ad-hoc signature is only
# trusted on the machine that produced it.
SIGN_IDENTITY="${SIGN_IDENTITY:--}"

UNIVERSAL=false
MAKE_DMG=false
INSTALL=false
for arg in "$@"; do
    case "$arg" in
        --universal) UNIVERSAL=true ;;
        --dmg)       MAKE_DMG=true ;;
        --install)   INSTALL=true ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

if $UNIVERSAL; then
    echo "==> Building universal binary (arm64 + x86_64)"
    rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    mkdir -p target/universal
    lipo -create -output target/universal/ichi \
        target/aarch64-apple-darwin/release/ichi \
        target/x86_64-apple-darwin/release/ichi
    BINARY="target/universal/ichi"
else
    echo "==> Building release binary"
    cargo build --release
    BINARY="target/release/ichi"
fi

echo "==> Assembling ${BUNDLE}"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
cp "$BINARY" "$BUNDLE/Contents/MacOS/ichi"

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
    <!-- 13.0 for SMAppService, which backs the Launch at Login toggle. -->
    <key>LSMinimumSystemVersion</key>    <string>13.0</string>

    <!-- Stealth Mode: no Dock icon, no Cmd-Tab entry, never steals focus. -->
    <key>LSUIElement</key>               <true/>
</dict>
</plist>
PLIST

if [[ -f icon.png ]]; then
    echo "==> Generating icon"
    ICONSET_DIR="$(mktemp -d)"
    ICONSET="$ICONSET_DIR/ichi.iconset"
    mkdir -p "$ICONSET"
    for size in 16 32 128 256 512; do
        sips -z $size $size icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null 2>&1
        sips -z $((size * 2)) $((size * 2)) icon.png \
             --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null 2>&1
    done
    iconutil -c icns "$ICONSET" -o "$BUNDLE/Contents/Resources/ichi.icns"
    rm -rf "$ICONSET_DIR"
fi

echo "==> Signing (${SIGN_IDENTITY})"
if [[ "$SIGN_IDENTITY" == "-" ]]; then
    codesign --force --deep --sign - "$BUNDLE"
else
    # Hardened runtime is required for notarization.
    codesign --force --deep --options runtime --timestamp \
        --sign "$SIGN_IDENTITY" "$BUNDLE"
fi
codesign --verify --verbose=2 "$BUNDLE" 2>&1 | sed 's/^/    /'

if $MAKE_DMG; then
    ./package.sh
fi

echo
echo "Built ${BUNDLE}"
lipo -archs "$BUNDLE/Contents/MacOS/ichi" | sed 's/^/    architectures: /'

if $INSTALL; then
    # Ichi is a background agent with no window, so there is nothing on screen
    # to tell you which build you are talking to. Without killing the old
    # process first, double-clicking the app appears to do nothing at all and
    # you end up testing the previous build — so always stop, replace, relaunch.
    echo
    echo "==> Installing to /Applications"
    if pgrep -qf "${APP_NAME}.app/Contents/MacOS/ichi"; then
        echo "    stopping running instance"
        pkill -f "${APP_NAME}.app/Contents/MacOS/ichi" || true
        sleep 1
    fi
    rm -rf "/Applications/${APP_NAME}.app"
    cp -R "$BUNDLE" /Applications/
    open "/Applications/${APP_NAME}.app"
    sleep 1
    if pgrep -qf "${APP_NAME}.app/Contents/MacOS/ichi"; then
        echo "    running: /Applications/${APP_NAME}.app"
    else
        echo "    warning: did not start — run it manually to see the error" >&2
    fi
else
    echo
    echo "Next:"
    echo "  ./bundle.sh --install                    # install to /Applications and relaunch"
    echo
    echo "Or by hand:"
    echo "  1. cp -R ${BUNDLE} /Applications/"
    echo "  2. open /Applications/${APP_NAME}.app    # prompts for Accessibility"
    echo "  3. Grant it in System Settings > Privacy & Security > Accessibility"
    echo "  4. Relaunch — macOS only applies the grant to a fresh process"
fi
