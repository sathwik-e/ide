#!/usr/bin/env bash
# Build Orion.app (release) into target/release/bundle/.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

APP="target/release/bundle/Orion.app"
rm -rf "$APP" target/release/bundle/IDE.app target/release/bundle/Eclipse.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/orion "$APP/Contents/MacOS/orion"
[ -f assets/AppIcon.icns ] && cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Orion</string>
  <key>CFBundleDisplayName</key><string>Orion</string>
  <key>CFBundleIdentifier</key><string>dev.orion-ide.app</string>
  <key>CFBundleExecutable</key><string>orion</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSDocumentsFolderUsageDescription</key><string>Orion opens and edits the projects you keep in Documents.</string>
  <key>NSDesktopFolderUsageDescription</key><string>Orion opens and edits the projects you keep on the Desktop.</string>
  <key>NSDownloadsFolderUsageDescription</key><string>Orion opens and edits files you have downloaded.</string>
  <key>NSRemovableVolumesUsageDescription</key><string>Orion opens projects stored on external drives.</string>
  <key>NSNetworkVolumesUsageDescription</key><string>Orion opens projects stored on network drives.</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSAppTransportSecurity</key>
  <dict><key>NSAllowsArbitraryLoads</key><true/><key>NSAllowsLocalNetworking</key><true/></dict>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Source Code and Text</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key>
      <array><string>public.text</string><string>public.source-code</string><string>public.script</string><string>public.data</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Any File</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key>
      <array><string>public.item</string><string>public.content</string></array>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Folder</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key>
      <array><string>public.folder</string><string>public.directory</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
# Tell Launch Services about the bundle so Finder offers it in "Open With".
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
[ -x "$LSREG" ] && "$LSREG" -f "$APP" >/dev/null 2>&1 || true
echo "Built $APP"
echo "Install:  cp -R \"$APP\" /Applications/"
