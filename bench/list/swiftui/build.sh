#!/bin/sh
# build.sh — ListBench.app (the SwiftUI port, App.swift) for the simulator (swiftui/build-sim/,
# ad-hoc signed) and the phone (swiftui/build/, unsigned: resign.sh signs it), into
# target/bench/list (BENCH_WORK overrides). No Xcode project: swiftc and a written Info.plist.
set -e
B=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$B/../../.." && pwd)
W=${BENCH_WORK:-$ROOT/target/bench/list}/swiftui
assemble() { # $1 = out dir, $2 = platform name, $3 = sdk platform
  APP=$1/ListBench.app
  rm -rf "$APP"; mkdir -p "$APP"
  cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>ListBench</string>
<key>CFBundleIdentifier</key><string>dev.exact.listbench.swiftui</string>
<key>CFBundleName</key><string>SwiftUI Bench</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleSupportedPlatforms</key><array><string>$2</string></array>
<key>DTPlatformName</key><string>$3</string>
<key>MinimumOSVersion</key><string>17.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer></array>
<key>UILaunchScreen</key><dict/>
<key>UIApplicationSupportsIndirectInputEvents</key><true/>
<key>CADisableMinimumFrameDurationOnPhone</key><true/>
</dict></plist>
PLIST
}
assemble "$W/build-sim" iPhoneSimulator iphonesimulator
xcrun -sdk iphonesimulator swiftc -parse-as-library -O -whole-module-optimization \
  -target arm64-apple-ios17.0-simulator "$B/App.swift" -o "$W/build-sim/ListBench.app/ListBench"
codesign -s - -f "$W/build-sim/ListBench.app"
assemble "$W/build" iPhoneOS iphoneos
xcrun -sdk iphoneos swiftc -parse-as-library -O -whole-module-optimization \
  -target arm64-apple-ios17.0 "$B/App.swift" -o "$W/build/ListBench.app/ListBench"
echo "built $W/build-sim/ListBench.app $W/build/ListBench.app"
