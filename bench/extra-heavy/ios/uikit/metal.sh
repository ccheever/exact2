#!/bin/bash
# metal.sh — compiles Shader.metal into obj/default-<sdk>.metallib for iphoneos and iphonesimulator. Uses this
# Mac's Metal Toolchain when it has one (`xcodebuild -downloadComponent MetalToolchain` installs it, ~840 MB);
# otherwise BENCH_METAL_HOST=<ssh host>, a Mac that has it, compiles them and they are copied back.
set -e
cd "$(dirname "$0")"
mkdir -p obj
SDKS="iphoneos:ios17.0 iphonesimulator:ios17.0-simulator"
if xcrun -sdk iphoneos metal --version >/dev/null 2>&1; then
  for s in $SDKS; do sdk=${s%%:*}; tgt=${s#*:}
    xcrun -sdk $sdk metal -c Shader.metal -o obj/sh-$sdk.air -target air64-apple-$tgt
    xcrun -sdk $sdk metallib obj/sh-$sdk.air -o obj/default-$sdk.metallib
  done
else
  M=${BENCH_METAL_HOST:?no Metal Toolchain here: install it, or set BENCH_METAL_HOST to a Mac that has it}
  T=xheavy-metal-$(basename "$PWD")
  ssh $M "mkdir -p /tmp/$T"
  scp -q Shader.metal $M:/tmp/$T/
  ssh $M "cd /tmp/$T && export DEVELOPER_DIR=\${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer} &&
    for s in $SDKS; do sdk=\${s%%:*}; tgt=\${s#*:};
      xcrun -sdk \$sdk metal -c Shader.metal -o sh-\$sdk.air -target air64-apple-\$tgt &&
      xcrun -sdk \$sdk metallib sh-\$sdk.air -o default-\$sdk.metallib; done"
  scp -q $M:/tmp/$T/default-iphoneos.metallib $M:/tmp/$T/default-iphonesimulator.metallib obj/
fi
echo metallibs
