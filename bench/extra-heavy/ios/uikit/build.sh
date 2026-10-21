#!/bin/bash
# Builds XHeavyUIKit.app for the iPad simulator (build-sim/, ad-hoc signed; SIM=1) and the device (build/, unsigned).
# Inputs: ../../data and ../../vendor/lottie-ios (../../prepare.sh; a static Lottie module per SDK, compiled once
# into obj/). default.metallib is compiled by metal.sh (the Metal Toolchain, here or on BENCH_METAL_HOST).
set -e
cd "$(dirname "$0")"
DATA=../../data
LOT=../../vendor/lottie-ios/Sources
SRC="App.swift Feed.swift Media.swift Cells.swift Text.swift Live.swift Shader.swift Nested.swift"

rm -rf obj/Assets.xcassets; mkdir -p obj/Assets.xcassets
echo '{"info":{"version":1,"author":"xcode"}}' > obj/Assets.xcassets/Contents.json
for f in $DATA/svg/*.svg; do
  n=$(basename $f .svg); d=obj/Assets.xcassets/$n.imageset; mkdir -p $d; cp $f $d/
  echo "{\"images\":[{\"filename\":\"$n.svg\",\"idiom\":\"universal\"}],\"info\":{\"version\":1,\"author\":\"xcode\"},\"properties\":{\"preserves-vector-representation\":true}}" > $d/Contents.json
done
FONTS=$(cd $DATA/fonts && ls *.ttf | sed 's|.*|<string>&</string>|' | tr -d '\n')

assemble() { # $1 out dir, $2 platform name, $3 sdk, $4 target
  APP=$1/XHeavyUIKit.app
  rm -rf "$APP"; mkdir -p "$APP"
  cp $DATA/feed.json $DATA/embed.html $DATA/images/*.jpg $DATA/anim/* $DATA/video/*.mp4 $DATA/fonts/*.ttf Shader.metal "$APP/"
  cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>XHeavyUIKit</string>
<key>CFBundleIdentifier</key><string>dev.exact.xheavy.uikit</string>
<key>CFBundleName</key><string>XHeavy UIKit</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleSupportedPlatforms</key><array><string>$2</string></array>
<key>DTPlatformName</key><string>$3</string>
<key>MinimumOSVersion</key><string>17.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array><key>UIRequiresFullScreen</key><true/>
<key>UISupportedInterfaceOrientations~ipad</key><array><string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationPortraitUpsideDown</string><string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string></array>
<key>UILaunchScreen</key><dict/>
<key>UIUserInterfaceStyle</key><string>Light</string>
<key>UIAppFonts</key><array>$FONTS</array>
<key>UIApplicationSupportsIndirectInputEvents</key><true/>
<key>CADisableMinimumFrameDurationOnPhone</key><true/>
<key>UIApplicationSceneManifest</key><dict>
  <key>UIApplicationSupportsMultipleScenes</key><false/>
  <key>UISceneConfigurations</key><dict><key>UIWindowSceneSessionRoleApplication</key><array><dict>
    <key>UISceneConfigurationName</key><string>Default</string>
    <key>UISceneDelegateClassName</key><string>SceneDelegate</string>
  </dict></array></dict>
</dict>
</dict></plist>
PLIST
  xcrun actool obj/Assets.xcassets --compile "$APP" --platform $3 --minimum-deployment-target 17.0 \
    --target-device ipad --target-device iphone --output-partial-info-plist obj/assets-$3.plist >/dev/null
  if [ ! obj/default-$3.metallib -nt Shader.metal ]; then ./metal.sh || echo "metal.sh failed: the app will compile Shader.metal at runtime"; fi
  [ -f obj/default-$3.metallib ] && [ obj/default-$3.metallib -nt Shader.metal ] && cp obj/default-$3.metallib "$APP/default.metallib"
  L=obj/lottie-$3
  if [ ! -f $L/libLottie.a ]; then
    if [ -f ../swiftui/obj/lottie-$3/libLottie.a ]; then mkdir -p obj; cp -R ../swiftui/obj/lottie-$3 obj/
    else
      mkdir -p $L
      xcrun -sdk $3 swiftc -O -whole-module-optimization -swift-version 5 -parse-as-library -target arm64-apple-$4 \
        -module-name Lottie -emit-library -static -emit-module -emit-module-path $L/Lottie.swiftmodule \
        -o $L/libLottie.a $(find $LOT -name '*.swift')
    fi
  fi
  xcrun -sdk $3 swiftc -parse-as-library -O -whole-module-optimization -module-name XHeavyUIKit -target arm64-apple-$4 \
    -I $L -L $L -lLottie $SRC -o "$APP/XHeavyUIKit"
}
if [ "$SIM" = 1 ]; then
  assemble build-sim iPhoneSimulator iphonesimulator ios17.0-simulator
  codesign -s - -f build-sim/XHeavyUIKit.app
fi
[ "$SIMONLY" = 1 ] || assemble build iPhoneOS iphoneos ios17.0
echo built
