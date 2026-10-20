# An SVG image asset draws blank on Apple

**Status:** Open
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

## Partly fixed (2026-10-19)

The compiler warns `lower-image-svg` on a literal image source naming an `.svg` file (templates and either arm of a choice included; `symbol:` and `data:` sources excepted), naming the inline `svg` element and the icon token as the drawing that works everywhere; `contract/cli/tests/it/warnings.rs::an_svg_image_source_is_named_as_blank_on_apple`. Drawing an SVG image source on Apple through the vector scene is not built: `exact-svg-raster` holds mask coverage and filters only, and the Linux painter's SVG rasterizer is not a crate the Apple host can take.
