# Apple Rust objects target the newest SDK

**Status:** Open
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
