#!/bin/bash
# build.sh [--run] — compile Sources/*.swift for the iOS simulator into build/App.app, with Resources/* at the
# bundle root, install it on $SIM (a simulator udid) and, with --run, launch it.
# app.env names the app: APP_NAME (the home-screen name) and APP_ID (the bundle id).
set -euo pipefail
cd "$(dirname "$0")"
source ./app.env
: "${SIM:?set SIM to the simulator udid you were given}"
APP=build/App.app
rm -rf "$APP"; mkdir -p "$APP"
[ -d Resources ] && find Resources -mindepth 1 -maxdepth 1 -exec cp -R {} "$APP/" \;
cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>App</string>
<key>CFBundleIdentifier</key><string>$APP_ID</string>
<key>CFBundleName</key><string>$APP_NAME</string>
<key>CFBundleDisplayName</key><string>$APP_NAME</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleSupportedPlatforms</key><array><string>iPhoneSimulator</string></array>
<key>DTPlatformName</key><string>iphonesimulator</string>
<key>MinimumOSVersion</key><string>26.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer></array>
<key>UILaunchScreen</key><dict/>
<key>UIApplicationSupportsIndirectInputEvents</key><true/>
<key>CADisableMinimumFrameDurationOnPhone</key><true/>
<key>UISupportedInterfaceOrientations</key><array><string>UIInterfaceOrientationPortrait</string></array>
${EXTRA_PLIST:-}
</dict></plist>
PLIST
xcrun -sdk iphonesimulator swiftc -parse-as-library -Onone -g -target arm64-apple-ios26.0-simulator \
  Sources/*.swift -o "$APP/App"
codesign -s - -f "$APP" >/dev/null 2>&1
xcrun simctl boot "$SIM" 2>/dev/null || true
xcrun simctl install "$SIM" "$APP"
[ "${1:-}" = --run ] && xcrun simctl launch "$SIM" "$APP_ID" >/dev/null && echo "launched $APP_ID"
echo "built $APP"
