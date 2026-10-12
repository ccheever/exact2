# Qualify platform-specific carrier code in the async verification lane

**Status:** Open
**Systems:** Windows carrier, Linux KMS/input, async verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** rules/DEFERRED.md scoped Windows consumers; scripts/async.mjs

The repository's asynchronous workspace build runs on macOS. It compiles shared native code, but excludes the Windows window module and Linux-only display/input code. Tier 2 adds tvOS only. There is no repository-owned Windows/Linux qualification step in the current lane.

`host/windows/src/lib.rs` compiles `window` only under cfg(windows); other platforms compile a small “requires a Windows target” stub. Linux KMS/DRM and device input are also selected by target OS. A green root/workspace build on this machine therefore cannot detect syntax/API failures in those branches, let alone qualify a native window or device.

The Windows work remains scoped to the admitted Skirmish/Windows Desk consumers, and general Windows/Android work is deferred. This issue asks to verify code already shipped for those consumers, not broaden product scope.

Add target-native verification through the existing asynchronous fleet/lane: build the admitted Windows consumers and exercise their carrier; build/test Linux-qualified code on Linux, with headless checks plus a bounded device smoke where needed. If such evidence is already produced outside this repo, link and integrate its failures into the existing lane instead of duplicating infrastructure.

Acceptance: changing a cfg(windows) or cfg(target_os = "linux") carrier branch produces a corresponding platform build result. A deliberate compile failure is detected and attributed. Publish which runtime/device behavior remains unqualified; do not treat the macOS stub build as platform proof.

## 2026-10-11 lane result

Tier 2 now cross-checks `exact-linux` with `caltrain-linux` and `exact-windows`
with `windows-desk` on their real targets. Both checks passed on this Mac with
Zig supplying C and the Rust target libraries installed. This proves those
source branches compile. Window presentation, KMS, and input devices still need
native-machine runs.

**Remains open:** select and qualify the other Linux app crates. The all-app
cross-check reached `ibex2`'s target C++ binding after the pinned Linux Hermes
bundle and frozen Bun dependencies were installed, then failed because the Mac
has no `x86_64-linux-gnu-g++`. A target `CXX` setting or a stub-engine compile
may resolve it, but neither was verified within the three-round fix limit.
