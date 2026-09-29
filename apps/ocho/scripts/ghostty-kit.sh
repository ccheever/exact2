#!/bin/bash
# Rebuild libghostty for macOS 14 (so Exact's deployment target matches), then
# compose the xcframework's macOS slice from the thin macOS arm64 archives,
# each repacked with Apple's libtool (Xcode 26's ld refuses zig's archives:
# "64-bit mach-o member not 8-byte aligned"), and their members made readable.
set -eu
export PATH="/Users/eliot/Developer/toolchains/xcrun-shim:/Users/eliot/Developer/toolchains/zig-0.15.2:/opt/homebrew/bin:$PATH"
cd /Users/eliot/Developer/ghostty
[ "${SKIP_ZIG:-}" = 1 ] || { rm -rf .zig-cache; zig build -Demit-macos-app=false -Dxcframework-target=native -Dtarget=aarch64-macos.14.0 -Doptimize=ReleaseFast; }
rm -rf /Users/eliot/Developer/ghostty/macos/GhosttyKit.xcframework/macos-arm64_x86_64
D=/Users/eliot/Developer/ghostty/macos/GhosttyKit.xcframework/macos-arm64
mkdir -p "$D/Headers"; cp -R include/ "$D/Headers/"
find "$D" -maxdepth 1 -name "*.a" -delete
R=$(mktemp -d)
find .zig-cache/o -name "*.a" | while read -r f; do
  plat=$(xcrun otool -l "$f" 2>/dev/null | grep -A2 LC_BUILD_VERSION | grep platform | head -1 | awk '{print $2}' || true)
  arch=$(xcrun lipo -info "$f" 2>/dev/null | sed 's/.*: //')
  name=$(basename "$f")
  [ "$plat" = "1" ] && [ "$arch" = "arm64" ] || continue
  case "$name" in libghostty-fat.a) continue;; esac
  rm -rf "$R"/* && (cd "$R" && ar x "/Users/eliot/Developer/ghostty/$f" && chmod 644 *.o)
  if [ -e "$D/$name" ]; then echo "duplicate $name from $f (kept first)"; continue; fi
  xcrun libtool -static -o "$D/$name" "$R"/*.o
  echo "packed $name from $f"
done
ls -la "$D"
xcrun nm -g "$D/libghostty.a" | grep -c "T _ghostty_"
