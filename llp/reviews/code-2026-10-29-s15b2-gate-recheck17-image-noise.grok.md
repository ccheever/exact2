# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck17-image-noise, 2026-10-29 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree of the branch's commit under review (`6ab243fa77`).
- **Method:** one brief (sha256 `b3d98698b7a921d147d589603b0294d42b60499f0f902c9584ca5c7f1f0d2dcd`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** Fixed in 3268b5364b: the noise boxes' crop paths listed box by box; image rects clipped to the screen; the 1500-line cap as caps counts it; the rule's order and host restriction tested.

## Brief

Narrow re-check of ONE commit, `git show HEAD` on the s15b2 gate branch (x2/s15b2-gate), touching scripts/stage12-gate.mjs, scripts/stage12-named.mjs and scripts/stage12-named.test.mjs. Read-only: no checkout, restore, stash, reset or edits.

theme2's ruling (22:01), the rule to implement: on web-wasm ONLY, inside Image nodes' boxes laid out at the same rect on both sides, differences of at most 4 levels are image-raster noise, counted and listed per shot with box and max level; anything over 4 or outside an image box still fails; the record lists crop paths for the noise boxes.
Check:
- pixelDiff's new `images` parameter: order of rules (allowlist first, then a named box takes every pixel, then the image rule, then the web edge floor); d <= 4 inside a box is not counted in `differ`; d = 5 inside is; d <= 4 outside every box still differs unless the edge floor takes it; the returned shape for callers without images/named is unchanged;
- imageBoxes: only type 'Image' nodes with size, from the whole layout; the `at` key equality (exact rect to 0.01 pt) and the pixel rect covering the node (floor/ceil);
- sameImageBoxes: only boxes both sides lay out alike; duplicates;
- comparePixels: only host 'web-wasm' passes boxes; `imageNoise` only boxes with count; names/png set when there is noise so crops are written (namedCrops gets image-noise-<rect> pseudo-names); does adding `names: []` when there are no named entries break namedReport or anything reading `x.names`?;
- the record's new evidence line; the 1500-line cap on stage12-gate.mjs (it should be exactly 1500);
- the test covers both sides of each rule.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

`6ab243fa` is one over the line cap, and the new record line does not list the crop paths. The pixel rule itself matches the 22:01 ruling.

**P2** — `scripts/stage12-gate.mjs:557` (also the new line at `scripts/stage12-gate.mjs:291`). The caps check counts `split('\n').length` (`scripts/caps.mjs:165`) and fails above 1500. The parent was 1499. This commit adds two lines, so `scripts/stage12-gate.mjs` is 1501.

**P2** — `scripts/stage12-gate.mjs:557`. Crops are written (`scripts/stage12-gate.mjs:519`) as `<shot>-image-noise-<x-y-w-h>-before.png` and `-after.png`, but the evidence line never prints those paths. It says "the directory above (`image-noise-<rect>`)" and then prints the rect with commas (`[32, 248, 42, 43]`). The named-box lines at `scripts/stage12-gate.mjs:556` list the real basenames. The directory path is only in the named-changes sentence at `scripts/stage12-gate.mjs:552`, and that sentence's file count now includes the noise crops.

**P3** — `scripts/stage12-named.test.mjs:212`. The new test locks both sides of the threshold (4 inside is noise, 5 inside differs, 3 outside differs), Image versus View and zero size, one floor/ceil rect, and same-versus-moved. It does not lock the rest of the order (allowlist, then a named box, then the image rule, then the web edge floor), the 0.01 `at` boundary, duplicate `at` keys, `comparePixels` passing boxes only for `web-wasm`, dropping boxes with `count` 0, or the `image-noise-<rect>` crops.

What holds: `pixelDiff` (`scripts/stage12-named.mjs:52`) skips an allowlisted pixel, then gives every remaining pixel in a named box to that name, then treats `d <= 4` inside an image box as noise (`differ` unchanged, `count` and `max` updated), then applies the edge floor. `d = 5` inside is counted. Callers that pass no `images` and no `named` still get `{ differ, total }` only. `imageBoxes` (`scripts/stage12-named.mjs:73`) keeps `type === 'Image'` nodes with `w > 0` and `h > 0` from `wholeLayout`, keys `at` with `toFixed(2)`, and covers the node with floor/ceil. `sameImageBoxes` (`scripts/stage12-named.mjs:82`) keeps a rect only when both sides share that key, and the `Map` collapses duplicate keys. `comparePixels` (`scripts/stage12-gate.mjs:290`) passes boxes only for `web-wasm`, and `imageNoise` keeps boxes with a count. `names: []` does not break `namedReport` or the named-box lines: both read `c.names ?? []`, so an empty list adds no hits and prints no named rows. The empty array is only there so `namedCrops` runs; the pseudo-names are not stored on `x.names`.

VERDICT: CHANGES
