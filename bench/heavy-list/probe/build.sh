#!/bin/bash
# build.sh — the simulator dylib (probe-sim.dylib) and the iPhone one (probe-ios.dylib)
set -e
B=$(cd "$(dirname "$0")" && pwd)
FW="-framework UIKit -framework QuartzCore -framework CoreGraphics -framework Foundation"
xcrun -sdk iphonesimulator clang -dynamiclib -fobjc-arc -O2 -Wall -target arm64-apple-ios17.0-simulator $FW \
  "$B/probe.m" -o "$B/probe-sim.dylib" -install_name @rpath/probe.dylib
xcrun -sdk iphoneos clang -dynamiclib -fobjc-arc -O2 -Wall -target arm64-apple-ios17.0 $FW \
  "$B/probe.m" -o "$B/probe-ios.dylib" -install_name @rpath/probe.dylib
echo built
