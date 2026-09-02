# Apple Rust objects target the newest SDK

**Status:** Closed
**Resolution:** Fixed: SwiftPM leaves a host SDKROOT in the tool environment and drives clang with --sysroot, which Darwin clang does not read; -Xswiftc -Xclang-linker -isysroot and xcrun --sdk close it, and the mixed-target gate now covers swift and xcrun as well as cargo
**Systems:** Apple build, ibex2
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1008

`host/apple/build.mjs:189-209` gives deployment triples and SDKs to Swift but
runs Cargo without the corresponding deployment environment. Both advertised
builds therefore link mixed-version objects:

- macOS: ibex2 `darwin_http.o` and `darwin_keychain.o` are marked macOS 26.5
  while the presenter links for macOS 14.0.
- iOS simulator: clang reports a MacOSX sysroot while targeting iPhone, and
  the same objects are marked iOS-simulator 26.5 while linking for 17.0.

These are reproducible linker warnings from `node host/apple/build.mjs` and
`node host/apple/build.mjs --ios`; the final artifact does not establish that
the declared minimum OS is actually supported.

Fix the Cargo/native dependency environment alongside the Swift triple:
deployment target, target SDK root, and target-aware C compiler flags for
macOS, simulator, and device. Add a clean-build check that treats these linker
and incompatible-sysroot warnings as failures.

## Attempted resolution (2026-09-01)

Cargo now receives the selected SDK root and the macOS 14/iOS 17 deployment
target, and its output is rejected on mixed-target diagnostics. Inspection of
all archive objects and the final simulator binary confirms iOS-simulator
platform metadata with minimum OS 17 or older. macOS builds cleanly. Three iOS
build rounds (deployment environment, explicit compiler sysroot, then explicit
linker sysroot) still leave SwiftPM's linker-driver warning `using sysroot for
'MacOSX' but targeting 'iPhone'`, despite its verbose command naming the iPhone
Simulator SDK and arm64 iOS 17 simulator triple and the resulting binary
carrying the correct load-command metadata. Per the repository's three-round
fix limit, this valid ticket remains open rather than suppressing the warning;
the remaining work is to isolate the SwiftPM/Xcode 26.5 diagnostic and make the
Swift build output fail closed once it can run warning-free.

## Resolution (2026-09-02)

The three earlier rounds aimed at the wrong flag. The warning is not about the
SDK that was chosen; it is about the one clang falls back to when nobody names
it at the linker driver.

**Root cause.** SwiftPM compiles `Package.swift` for the *host* first and
leaves `SDKROOT` naming the MacOSX SDK in the environment of every tool it then
spawns. Its link step drives clang with `--sysroot`, which is not the flag
clang reads on Darwin: with no `-isysroot` of its own, clang takes `$SDKROOT`
and links an iPhone target against a MacOSX sysroot. Reduced to two commands:

```
SDKROOT=<MacOSX26.5.sdk> clang t.c --sysroot <iPhoneSimulator26.5.sdk> \
    --target=arm64-apple-ios17.0-simulator     → the warning
… plus -isysroot <iPhoneSimulator26.5.sdk>     → clean
```

`-Xcc` reached only compiles and `-Xlinker -syslibroot` reached ld, which is
why both earlier rounds left the diagnostic standing while every other piece of
evidence — the verbose command line, the load commands in the binary — said the
build was correct. It was; only the driver's fallback was not.

The second warning, the one printed after `Build complete!`, was the webarm
`swiftc` with the same cause from a different direction: a bare `xcrun` exports
the default macosx `SDKROOT` for the tool it runs.

**The fix**, all in `host/apple/build.mjs`:

- `-Xswiftc -Xclang-linker -Xswiftc -isysroot -Xswiftc -Xclang-linker -Xswiftc
  <sdk>` on the iOS `swift build`. `-Xclang-linker` is the hook that reaches
  the clang the link step drives; `swift build` rejects it directly because it
  is a swiftc flag, so it goes through `-Xswiftc`.
- `xcrun --sdk <sdkName> swiftc` for the webarm arm.
- The mixed-target gate this ticket asked for, generalized from cargo to every
  Apple toolchain invocation (`runAppleCargo` → `runApple`, now wrapping
  `swift` and `xcrun` as well as `cargo`). It fails the build on
  `using sysroot for …`, an incompatible sysroot, or an object built for a
  newer OS than is being linked, so a regression cannot pass silently again.

**Verified** by running: clean simulator build, clean macOS build, clean device
(`iphoneos`) `swift build`, all three with no warning of any kind. `vtool`
reports `IOSSIMULATOR minos 17.0` and `IOS minos 17.0`. `node
scripts/smoke.mjs ios` and `macos` both green; `caps` green.
