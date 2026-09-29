# Ocho

A client for [Fleet](https://github.com/eiiot/fleet): machines, sessions and
accounts in a sidebar, a Ctrl+N launcher with stable two-digit codes, and each
open session in a real terminal. One Contract source; macOS first, the web as
the dev loop.

- `app.contract` — the view.
- `data/` — the Rust source: every answer is a long native call (`{op, args}`)
  to the app's module; the `fleet` resource watches the module's `fleet` topic.
- `modules/apple/` — the Swift module. `Ocho.swift` runs `fleet snapshot
  --watch`, keeps the tabs (`tabs.json` in the app's data directory) and the
  launcher's slots (`launcher.json`, seeded from the existing desktop's
  `~/.local/share/fleet/desktop.json`), and answers the one-shot commands.
  `Terminal.swift` is `<ghostty-terminal>`: one libghostty surface per tab,
  kept while the tab is open, running `fleet attach` / `fleet run --attach` /
  `fleet connect`.
- `modules/web/index.js` — the page module: a fixture fleet and a placeholder
  terminal, so `bun host/web/dev.mjs --app ocho` shows the whole app.

## Building on macOS

The `fleet` CLI is found at `$FLEET_BIN`, the installed Ocho's copy
(`~/Applications/Ocho.app/Contents/Resources/bin/fleet`), `~/.local/bin/fleet`,
then `PATH`.

libghostty is linked from `modules/apple/GhosttyKit.xcframework`, a symlink to
a Ghostty checkout's `macos/GhosttyKit.xcframework` (ignored by git). Building
it needs Zig 0.15.2 and the Xcode Metal toolchain (`xcodebuild
-downloadComponent MetalToolchain`), with two workarounds for Xcode 26: an
`xcrun` shim on `PATH` that answers `--show-sdk-path` with a macOS 15 SDK (Zig
0.15.2 cannot link the 26.x SDK's libSystem), and each archive repacked with
Apple's `libtool` (Xcode 26's linker refuses Zig's archives, "member not 8-byte
aligned"). `host.macos.link` in `app.json` adds `-lc++` for the C++ inside
libghostty. The recipe, from the Ghostty checkout (v1.3.1), targeting macOS 14
so the slice matches Exact's deployment target:

```sh
zig build -Demit-macos-app=false -Dxcframework-target=native \
  -Dtarget=aarch64-macos.14.0 -Doptimize=ReleaseFast
D=macos/GhosttyKit.xcframework/macos-arm64
rm -rf macos/GhosttyKit.xcframework/macos-arm64_x86_64   # the fat slice zig wrote
mkdir -p "$D/Headers" && cp -R include/ "$D/Headers/" && find "$D" -maxdepth 1 -name '*.a' -delete
R=$(mktemp -d)
find .zig-cache/o -name '*.a' | while read -r f; do   # the thin macOS arm64 archives only
  plat=$(xcrun otool -l "$f" | grep -A2 LC_BUILD_VERSION | awk '/platform/ {print $2; exit}')
  [ "$plat" = 1 ] && [ "$(xcrun lipo -info "$f" | sed 's/.*: //')" = arm64 ] || continue
  name=$(basename "$f"); [ "$name" = libghostty-fat.a ] && continue
  [ -e "$D/$name" ] && continue                          # a duplicate name: the first wins
  rm -rf "$R"/* && (cd "$R" && ar x "$OLDPWD/$f" && chmod 644 *.o)
  xcrun libtool -static -o "$D/$name" "$R"/*.o
done
xcrun nm -g "$D/libghostty.a" | grep -c ' T _ghostty_'   # the surface API is present
```

```sh
bun host/apple/build.mjs ocho-apple --run
bun host/web/dev.mjs --app ocho
```
