# Shared UIKit HDR tests fail on the iOS 27 simulator

**Status:** Open
**Systems:** Apple host, HDR, UIKit verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-13
**Related:** 20261006-ios-shared-tests-excluded

Removing the iOS test runner's filename filter exposes these existing shared
HDR test failures on iPhone 18 Pro Max, iOS 27.0 (24A434), Xcode 27:

- `HDRColorTests.testADrawnHDRColourGetsAHalfFloatStore`
- `HDRColorTests.testDrawnGradientAndBoxGradientTakeTheNodesLimit`
- `HDRColorTests.testGradientLayersPreserveWideAndHDRStops`
- `HDRImageTests.testAGainMapRecoversLightAboveSDRWhite`
- `HDRImageTests.testTheSDRRenditionStaysWithinSDR`
- `PickerColorTests.testAnHDRPhotoKeepsItsGainMapAndProfile`
- `PickerColorTests.testOrientationIsAppliedNotCopied`
- `RasterColorTests.testEveryFixtureDecodesToItsClassStorageAndColors`

The colour tests expect RGBA16Float layer contents but observe RGBA8.
Gain-map decoding returns SDR/headroom 1 instead of HDR/headroom 4; the SDR
rendition tests retain PQ/HLG colour spaces instead of Display P3. Picker JPEG
conversion loses the gain map/HDR flag, and converting pq.heic throws a missing
output-file removal error.
The assertions are unchanged. Whether these are simulator limitations or host
bugs still needs qualification on a device; do not hide them by filename again.

Reproduction: `bun host/apple/build.mjs --test --ios --sim
793D239B-0A6E-48CB-9C12-E87C729207EB` in the iostooling worktree at base
`9db708c47` with the shared-selection fix. The first verification used a temporary
serial override and a deliberately failing selection canary in SurfaceControlTests;
neither is part of the fix. Raw output: `target/iostooling-verification/ios-canary.log`.

## Platform isolation, 2026-10-13

A standalone Swift executable using only QuartzCore and ImageIO reproduces
these failures on the booted iPhone 18 Pro Max simulator
`793D239B-0A6E-48CB-9C12-E87C729207EB`:

- Setting a fresh `CALayer.contentsFormat` to `.RGBA16Float` or `.gray8Uint`
  leaves `.RGBA8Uint` and prints `unrecognised property key`.
- Both `CGImageSourceCreateImageAtIndex` and
  `CGImageSourceCreateThumbnailAtIndex`, with
  `kCGImageSourceDecodeRequest: kCGImageSourceDecodeToHDR`, decode
  `gainmap-iso.jpg` and `gainmap-iso.heic` as Display P3 with headroom 1.
- Both APIs with `kCGImageSourceDecodeToSDR` keep PQ for `pq.heic` and
  `pq.avif`, and HLG for `hlg.heic`, with their HDR headroom. The same
  request converts `pq-cicp.png` to Display P3 with headroom 1.

The same probe on macOS honors both layer formats and recovers gain-map
headroom 4. This isolates simulator API behavior from Exact's callers; it
is not physical-device qualification. The assertions remain in the iOS
suite. The baseline XCTest failures are in
`target/uikittests-verification/baseline.log`; the native comparison is in
`hdr-probe-mac.log` beside it.
