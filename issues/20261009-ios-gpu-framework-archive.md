# iOS App Store archive: an app's GPU module ships as a loose dylib, and App Store Connect fails it (ITMS-90426)

**Status:** Open
**Systems:** host/apple, GPU delivery
**Severity:** P1
**Author:** SchroederNathan (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/334

## Current scope

Framework-wrap and sign app GPU and gpu.modules artifacts before bake hashing. Load framework binaries on iOS; keep development dylibs. Verify shipped digest, no loose dylibs, codesign, device rendering and store processing. The supplied patch/TestFlight evidence is reported, not newly verified.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

`bun host/apple/build.mjs --device <crate> --archive out.ipa` wraps six of the bundle's libraries as frameworks (`host/apple/build.mjs:1373`), because App Store Connect takes an embedded library only in that form (the comment at `build.mjs:381-382` names ITMS-90171 and ITMS-90432). The app's own GPU module is not one of the six. An app with a `gpu/` crate (`caltrain` and `weatherlight` in this repo) ships it as `Frameworks/libexact_gpu.dylib`, and each module under `gpu.modules` ships as a loose `libexact_gpu_<name>.dylib`.

App Store Connect accepts the upload, then fails it in processing with ITMS-90426: "Invalid Swift Support. The SwiftSupport folder is missing." Nothing in the bundle needs Swift support. Every binary loads Swift from `/usr/lib/swift`, none loads a Swift library through `@rpath`, and the minimum iOS is 17.0. The likely reason is Apple's rule that iOS accepts no loose dylib in a bundle except the Swift libraries Xcode embeds ([Apple Developer Forums 810546](https://developer.apple.com/forums/thread/810546)), and those come with a `SwiftSupport/` folder. Another tool that makes `.ipa` files without Xcode had the same error and cleared it by removing its loose `Frameworks/lib*.dylib` ([iOS Build Environment forum](https://pmbaty.com/iosbuildenv/help/thread.php?path=Problem+solving%2F&file=invalid-swift-support)). Apple does not document this check; the two uploads below are the evidence.

The gap is deliberate. The bake records each GPU module's SHA-256 after the module is signed (`build.mjs:879`), and `GpuModule.verify` (`host/apple/Sources/ExactSurfaces/GpuModule.swift:51`) refuses a file whose digest differs. `wrapFramework` runs `install_name_tool` and the bundle is signed again, and both change the bytes. So the store path leaves the GPU modules out of the wrap (`build.mjs:1373`) and out of the re-sign (`:1374`), and the iOS presenter loads only the loose name (`ExactSurfaces/GpuIOS.swift:225`), where the other modules go through `embeddedModule(framework:dylib:)` (`ExactKit/VideoModule.swift:404`). `--unsigned` already refuses an app with GPU modules (`build.mjs:668`). On main, an app with a `gpu/` crate has no build that App Store Connect accepts.

### Current and expected behavior

- Current: an `--archive` of an app with a `gpu/` crate has a loose `Frameworks/libexact_gpu.dylib` and no `SwiftSupport/`. App Store Connect fails it in processing with ITMS-90426.
- Expected: every library in the `.ipa`'s `Frameworks/` is a framework, the GPU module's baked digest is the digest of the bytes that ship, and the build passes App Store Connect processing.

### Reproduction and evidence

An app outside this repo with a `gpu/` crate, built on EAS with an App Store distribution profile and uploaded with `eas submit`. The two rows differ only by the patch described under "Constraints and related work".

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Store build, as on main | `EXACT_IDENTITY=… EXACT_PROFILE=… bun exact.mjs ios --device --archive out.ipa`, then `eas submit` | EAS macOS 26.6 image, Xcode 27.0 (27A266a), iPhoneOS 27.0 SDK, minimum iOS 17.0 | `c248ad723` | `Frameworks/` holds `ExactCanvasGpu.framework`, `ExactWeb.framework` and a loose `libexact_gpu.dylib`; no `SwiftSupport/`. Processing fails with ITMS-90426 | passes processing | `unzip -l out.ipa`; the binaries' load commands; App Store Connect's processing result |
| Same, patched | same, with the patch applied to the exact2 clone | same | `c248ad723` + patch | `Frameworks/ExactGpu.framework`, no loose dylib; processing passes and the build reaches TestFlight | (the reference) | App Store Connect |

The code involved is unchanged on main `57eeaa189`. The `build.mjs` wrap list and re-sign filter are the same, and `GpuIOS.swift` (moved to `ExactSurfaces` by `244ac708d`) still loads the loose name. Not run: an `--archive` of `weatherlight-apple` on main.

### Acceptance criteria

- An `--archive` of `weatherlight-apple` has no loose dylib: `unzip -l out.ipa | grep -E 'Frameworks/[^/]+\.dylib$'` prints nothing.
- A module declared under `gpu.modules` ships as a framework the same way.
- `codesign --verify --deep --strict` passes on the `.app`.
- On a device, the GPU canvas draws, and the console shows no "digest mismatch".
- The `.ipa` passes App Store Connect processing (no ITMS-90426, ITMS-90171 or ITMS-90432).
- Development builds keep the loose dylib.

### Constraints and related work

- The framework must be made before the bake takes the digest. Wrapping after the build breaks the load path or the digest.
- A working patch against `c248ad723` does this:
  - where the GPU product is signed (`build.mjs:879`), an `.ipa` build lays out `signed/ExactGpu.framework` (`ExactGpu_<name>.framework` for a declared module) with an `Info.plist` like `wrapFramework`'s, runs `install_name_tool -id @rpath/ExactGpu.framework/ExactGpu`, signs the framework directory, and returns its binary for the bake to digest;
  - the bundle copies that framework into `Frameworks/` instead of the loose file (`:1332-1333`), after it checks that the framework holds the bytes the bake digested, and the re-sign filter (`:1374`) skips the framework names;
  - the framework's bundle identifier replaces `_` with `-`, because a bundle identifier takes no underscore;
  - `GpuIOS.swift` builds the path with `embeddedModule(framework:dylib:)` and a new `GpuModule.frameworkName(artifact:)`;
  - `EXACT_GPU_FRAMEWORK=1` lays out a simulator build the same way, so the load path can be tried without a device.
- On main, the patch's two Swift hunks need the `ExactSurfaces` paths; the `build.mjs` hunks apply.
- A guard would keep this from coming back: the archive step refuses an `.ipa` with any loose dylib in `Frameworks/`.
- Related: `0e6fe12c8` (iOS store builds wrap the modules, SVG and canvas GPU dylibs), LLP 1009 (GPU modules).
