#!/usr/bin/env bash
# Build Eclipse.app (release) into target/release/bundle/.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

APP="target/release/bundle/Eclipse.app"
rm -rf "$APP" target/release/bundle/IDE.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/eclipse "$APP/Contents/MacOS/eclipse"
[ -f assets/AppIcon.icns ] && cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Eclipse</string>
  <key>CFBundleDisplayName</key><string>Eclipse</string>
  <key>CFBundleIdentifier</key><string>dev.eclipse-ide.app</string>
  <key>CFBundleExecutable</key><string>eclipse</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSAppTransportSecurity</key>
  <dict><key>NSAllowsArbitraryLoads</key><true/><key>NSAllowsLocalNetworking</key><true/></dict>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Source Code</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSItemContentTypes</key>
      <array><string>public.text</string><string>public.source-code</string><string>public.folder</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
echo "Built $APP"
echo "Install:  cp -R \"$APP\" /Applications/"
