#!/bin/bash
# build.sh — the probe and the layer diagnostic, for the simulator (*-sim.dylib) and a
# phone (*-ios.dylib), into target/bench/list (BENCH_WORK overrides).
set -e
B=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$B/../../.." && pwd)
W=${BENCH_WORK:-$ROOT/target/bench/list}; mkdir -p "$W"
FW="-framework UIKit -framework QuartzCore -framework CoreGraphics -framework Foundation"
for name in probe diag; do
  xcrun -sdk iphonesimulator clang -dynamiclib -fobjc-arc -O2 -Wall -target arm64-apple-ios17.0-simulator $FW \
    "$B/$name.m" -o "$W/$name-sim.dylib" -install_name @rpath/$name.dylib
  xcrun -sdk iphoneos clang -dynamiclib -fobjc-arc -O2 -Wall -target arm64-apple-ios17.0 $FW \
    "$B/$name.m" -o "$W/$name-ios.dylib" -install_name @rpath/$name.dylib
done
echo "built $W/{probe,diag}-{sim,ios}.dylib"
