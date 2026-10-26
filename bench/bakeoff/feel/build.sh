#!/bin/sh
# Builds the recorder for the simulator: target/feel/feel-sim.dylib under the repository root.
set -e
B=$(cd "$(dirname "$0")" && pwd)
OUT=$B/../../../target/feel
mkdir -p "$OUT"
xcrun -sdk iphonesimulator clang -dynamiclib -fobjc-arc -O2 -Wall -target arm64-apple-ios17.0-simulator \
  -framework UIKit -framework QuartzCore -framework CoreGraphics -framework Foundation \
  "$B/feel.m" -o "$OUT/feel-sim.dylib" -install_name @rpath/feel.dylib
