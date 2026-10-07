#!/bin/sh
# Builds a release-shaped bundle: Fixture.app with a dylib, an unsigned helper under
# Contents/Resources/assets, a fat (arm64+x86_64) helper under Contents/MacOS, and a nested
# unsigned Inner.app under Contents/Helpers.
set -e
D=$1; rm -rf "$D"; mkdir -p "$D"; cd "$D"
printf 'int main(void){return 0;}\n' > m.c
printf 'int f(void){return 1;}\n' > l.c
A=Fixture.app/Contents
mkdir -p $A/MacOS $A/Resources/assets $A/Helpers/Inner.app/Contents/MacOS
plist() { cat > "$1" <<P
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>$2</string>
<key>CFBundleIdentifier</key><string>$3</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
P
}
plist $A/Info.plist ExactMac com.example.fixture
plist $A/Helpers/Inner.app/Contents/Info.plist Inner com.example.fixture.inner
clang -o $A/MacOS/ExactMac m.c
clang -dynamiclib -o $A/MacOS/libexact_web.dylib l.c
clang -Wl,-no_adhoc_codesign -o $A/Resources/assets/helper m.c
clang -arch arm64 -arch x86_64 -Wl,-no_adhoc_codesign -o $A/MacOS/spawn-helper m.c
clang -Wl,-no_adhoc_codesign -o $A/Helpers/Inner.app/Contents/MacOS/Inner m.c
printf '{"channel":"stable"}\n' > $A/Resources/assets/distribution.json
rm m.c l.c
