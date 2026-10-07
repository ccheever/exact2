#!/bin/bash
# The README's AppKit recipe for the contextmenu binary alone (data keys, every module file, the test sources).
set -e
X=$(xcode-select -p); F="$X/Platforms/MacOSX.platform/Developer/Library/Frameworks"; L="$X/Platforms/MacOSX.platform/Developer/usr/lib"
R="$PWD/target/t3-tests"; O="$R/contextmenu"; mkdir -p "$O"
T3_DK="$R" "$HOME/.bun-1.4.2/bin/bun" -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_DK + "/ExactDataKeys.swift");'
xcrun swiftc -swift-version 5 -module-name T3contextmenuTests -F "$F" -I "$L" -L "$L" -Xlinker -rpath -Xlinker "$F" -Xlinker -rpath -Xlinker "$L" \
  host/apple/modules/ExactNativeModule.swift "$R/ExactDataKeys.swift" examples/t3-code/modules/apple/*.swift examples/t3-code/macos/tests/contextmenu/*.swift -o "$O/contextmenu-tests"
"$O/contextmenu-tests"
