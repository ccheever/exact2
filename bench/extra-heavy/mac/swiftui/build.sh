#!/bin/bash
# build.sh — builds build/XHeavy.app for macOS (arm64, deployment target 14.0, ad-hoc signed). Needs the Metal
# Toolchain (`xcodebuild -downloadComponent MetalToolchain`). Inputs: ../../data and ../../vendor/lottie-ios
# (../../prepare.sh).
set -e
cd "$(dirname "$0")"
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
DATA=../../data
LOT=../../vendor/lottie-ios/Sources
SRC="App.swift Kinds.swift Media.swift Nested.swift"
MINOS=${MINOS:-14.0}

rm -rf obj/Assets.xcassets; mkdir -p obj/Assets.xcassets
echo '{"info":{"version":1,"author":"xcode"}}' > obj/Assets.xcassets/Contents.json
for f in $DATA/svg/*.svg; do
  n=$(basename $f .svg); d=obj/Assets.xcassets/$n.imageset; mkdir -p $d; cp $f $d/
  echo "{\"images\":[{\"filename\":\"$n.svg\",\"idiom\":\"universal\"}],\"info\":{\"version\":1,\"author\":\"xcode\"},\"properties\":{\"preserves-vector-representation\":true}}" > $d/Contents.json
done

APP=build/XHeavy.app
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
R="$APP/Contents/Resources"
cp $DATA/feed.json $DATA/embed.html $DATA/images/*.jpg $DATA/anim/* $DATA/video/*.mp4 $DATA/fonts/*.ttf "$R/"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>XHeavy</string>
<key>CFBundleIdentifier</key><string>dev.exact.xheavy.swiftui</string>
<key>CFBundleName</key><string>XHeavy SwiftUI</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleSupportedPlatforms</key><array><string>MacOSX</string></array>
<key>LSMinimumSystemVersion</key><string>$MINOS</string>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSRequiresAquaSystemAppearance</key><true/>
<key>ATSApplicationFontsPath</key><string>.</string>
</dict></plist>
PLIST
xcrun actool obj/Assets.xcassets --compile "$R" --platform macosx --minimum-deployment-target $MINOS \
  --target-device mac --output-partial-info-plist obj/assets-macosx.plist >/dev/null
if [ ! obj/default-macosx.metallib -nt Shader.metal ]; then
  xcrun -sdk macosx metal -c Shader.metal -o obj/sh.air -target air64-apple-macos$MINOS
  xcrun -sdk macosx metallib obj/sh.air -o obj/default-macosx.metallib
fi
cp obj/default-macosx.metallib "$R/default.metallib"
L=obj/lottie-macosx
if [ ! -f $L/libLottie.a ]; then
  mkdir -p $L
  xcrun -sdk macosx swiftc -O -whole-module-optimization -swift-version 5 -parse-as-library -target arm64-apple-macos$MINOS \
    -module-name Lottie -emit-library -static -emit-module -emit-module-path $L/Lottie.swiftmodule \
    -o $L/libLottie.a $(find $LOT -name '*.swift')
fi
xcrun -sdk macosx swiftc -parse-as-library -O -whole-module-optimization -target arm64-apple-macos$MINOS \
  -I $L -L $L -lLottie $SRC -o "$APP/Contents/MacOS/XHeavy"
codesign -s - -f "$APP"
echo built
