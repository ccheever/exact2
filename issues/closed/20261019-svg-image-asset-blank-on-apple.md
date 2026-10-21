# An SVG image asset draws blank on Apple

**Status:** Closed
**Resolution:** Drawn as a picture on Apple (LLP 1011.001 stages 1 and 3) and Linux (stage 2); the build check is stage 4, queued
**Systems:** Apple host RasterLoader/RasterInput, image, docs
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

`image "app:/assets/wordmark.svg"` renders nothing on iOS, with no build diagnostic; inlining the paths as an `svg` node works.

## Cause

Every image source on Apple goes through ImageIO (`RasterInput.open`, `RasterMetadata.read`, `host/apple/Sources/ExactKit/RasterImage.swift:93-130`), which has no SVG decoder: the file gives no pixel size and fails as "not an image format this host decodes" (logged as `image deferred:`, and the node's `error` event), and the kernel measures the unknown axis as 0. LLP 1011 §4 declares SVG sources on Apple not built. The web draws them. No lint names an `.svg` image source.

## Fix

Draw an SVG image source on Apple through the SVG scene the `svg` node already uses (`exact-svg-raster` for a raster, or the vector scene at the image's box), as the web does; at least, diagnose an `.svg` image source at build time with the inline alternative.

## Fixed on Apple (2026-10-20)

LLP 1011.001 (reviewed by Astra and Opus, r2) and its stages 1 and 3: `host/svg-file` parses an SVG file into the kernel's own SVG nodes in a scratch kernel and resolves it with the kernel's scene resolver; the Apple raster loader sniffs an SVG by its bytes, takes its natural size from the root tag (D1's rule, with the viewBox deviation), plans at the drawn size (above the natural size, and under `object-fit: fill` at the box's own ratio) and renders the scene as an island is rendered into the plan's storage. Verified by `RasterImageTests.testAnSVGIsAPictureDrawnAtThePlannedSize` and a scratch macOS app whose wordmark (a class stylesheet, a gradient, dashes, a script) draws at four sizes and fits. Linux still draws it blank; the compiler's warning now says so and stays until Linux lands (the RFC's stage 2); iOS was not driven here.

## Fixed on Linux (2026-10-20)

LLP 1011.001 stage 2: the Linux raster loader sniffs an SVG by its bytes, plans it at the box's pixels and paints the resolved scene as an island is painted, on the decode worker, or on the presenter's thread when the picture has text (`host/linux/src/image/svg.rs`, `paint/picture.rs`; `image/svg_tests.rs`). The `lower-image-svg` warning is retired. What remains is the RFC's stage 4, the compiler's build check of a bundled file, queued.

## Partly fixed (2026-10-19)

The compiler warns `lower-image-svg` on a literal image source naming an `.svg` file (templates and either arm of a choice included; `symbol:` and `data:` sources excepted), naming the inline `svg` element and the icon token as the drawing that works everywhere; `contract/cli/tests/it/warnings.rs::an_svg_image_source_is_named_as_blank_on_apple`. Drawing an SVG image source on Apple through the vector scene is not built: `exact-svg-raster` holds mask coverage and filters only, and the Linux painter's SVG rasterizer is not a crate the Apple host can take.
