#!/bin/bash
# build.sh — probe-mac.dylib (arm64, macOS 14.0). Builds with this Mac's SDK (no Metal needed).
set -e
B=$(cd "$(dirname "$0")" && pwd)
xcrun -sdk macosx clang -dynamiclib -fobjc-arc -O2 -Wall -target arm64-apple-macos14.0 \
  -framework AppKit -framework QuartzCore -framework CoreGraphics -framework Foundation \
  "$B/probe.m" -o "$B/probe-mac.dylib"
codesign -s - -f "$B/probe-mac.dylib" >/dev/null 2>&1
echo built
