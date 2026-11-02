#!/bin/sh
# Builds HeavyBench.app for the simulator (build-sim/, ad-hoc signed) and the iPhone (build/, unsigned).
set -e
cd "$(dirname "$0")"
../prepare.sh
DATA=../data
assemble() { # $1 = out dir, $2 = platform name, $3 = sdk platform
  APP=$1/HeavyBench.app
  rm -rf "$APP"; mkdir -p "$APP"
  cp "$DATA/messages.json" "$APP/"
  cp "$DATA"/images/*.jpg "$APP/"
  cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>HeavyBench</string>
<key>CFBundleIdentifier</key><string>dev.exact.heavybench.swiftui</string>
<key>CFBundleName</key><string>Heavy SwiftUI</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleSupportedPlatforms</key><array><string>$2</string></array>
<key>DTPlatformName</key><string>$3</string>
<key>MinimumOSVersion</key><string>17.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array><key>UIRequiresFullScreen</key><true/>
<key>UILaunchScreen</key><dict/>
<key>UIApplicationSupportsIndirectInputEvents</key><true/>
<key>CADisableMinimumFrameDurationOnPhone</key><true/>
</dict></plist>
PLIST
}
assemble build-sim iPhoneSimulator iphonesimulator
xcrun -sdk iphonesimulator swiftc -parse-as-library -O -whole-module-optimization \
  -target arm64-apple-ios17.0-simulator App.swift -o build-sim/HeavyBench.app/HeavyBench
codesign -s - -f build-sim/HeavyBench.app
assemble build iPhoneOS iphoneos
xcrun -sdk iphoneos swiftc -parse-as-library -O -whole-module-optimization \
  -target arm64-apple-ios17.0 App.swift -o build/HeavyBench.app/HeavyBench
echo built
