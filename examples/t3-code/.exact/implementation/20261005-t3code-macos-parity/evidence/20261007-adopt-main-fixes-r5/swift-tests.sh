#!/bin/bash
# The README's AppKit/XCTest recipe for the named test directories. Run from the repository root;
# exits non-zero when any binary fails to build or run.
set -u
X=$(xcode-select -p); F="$X/Platforms/MacOSX.platform/Developer/Library/Frameworks"; L="$X/Platforms/MacOSX.platform/Developer/usr/lib"
R="$PWD/target/t3-tests"; mkdir -p "$R"; export T3_APP_DIR="$PWD/examples/t3-code"
T3_DK="$R" bun -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_DK + "/ExactDataKeys.swift");' || exit 1
status=0
for n in "$@"; do
  d=examples/t3-code/macos/tests/$n/; O="$R/$n"; mkdir -p "$O"
  M=$(ls examples/t3-code/modules/apple/*.swift); case $n in composer|menus|r5-panels) M=$(echo "$M" | grep -v /T3Module);; esac
  xcrun swiftc -swift-version 5 -module-name "T3$(echo $n | tr -d -)Tests" -F "$F" -I "$L" -L "$L" -Xlinker -rpath -Xlinker "$F" -Xlinker -rpath -Xlinker "$L" \
    host/apple/modules/ExactNativeModule.swift "$R/ExactDataKeys.swift" $M "$d"*.swift -o "$O/$n-tests" || { echo "== $n: build failed"; status=1; continue; }
  T3_COMPOSER_TEST_DIR="$O" T3_MENUS_TEST_DIR="$O" T3_PANELS_TEST_DIR="$O" T3_TERMINAL_TEST_DIR="$O" "$O/$n-tests" || { echo "== $n: failed"; status=1; }
done
exit $status
