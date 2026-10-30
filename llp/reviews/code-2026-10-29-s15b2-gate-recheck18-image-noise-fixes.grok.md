# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck18-image-noise-fixes, 2026-10-29 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree of the branch's commit under review (`3268b5364b`).
- **Method:** one brief (sha256 `bb4d1900d8029e5ff250996d80cbd9370ce0069553b4ea96a8ac8d4a97c271c9`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes (Astra, P3); clean (Grok).
- **Disposition:** Fixed in c75cbf62af: an edge pixel inside an image box tested as the image rule's.

## Brief

Narrow re-check of ONE commit, `git show HEAD~1 HEAD` (the rule, then the fixes for its first re-check, whose findings were: noise crop paths missing from the record; partly-visible image rects cropped wrong; the 1500-line cap as caps counts it; thinner tests) on the s15b2 gate branch (x2/s15b2-gate), touching scripts/stage12-gate.mjs, scripts/stage12-named.mjs and scripts/stage12-named.test.mjs. Read-only: no checkout, restore, stash, reset or edits.

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

`pixelDiff` applies the rules in the required order: the allowlist, then a named box (every pixel in it), then the image rule, then the web edge floor. A difference of 4 or less inside a box is counted on that box and left out of `differ`; 5 inside is counted in `differ`; 4 or less outside every box still differs unless the edge floor takes it. Callers that pass no `images` and no `named` still get `{differ, total}` or `{differ, total, size}`.

`imageBoxes` takes only `Image` nodes with a positive size from the whole layout. The `at` key is the rect at hundredths of a point, so 0.01 apart does not match. The pixel rect is the floor/ceil coverage of the node, clipped to the viewport, and a box wholly off the screen is dropped. `sameImageBoxes` keeps a rect only when both sides share that key, and a repeated key once.

`comparePixels` passes boxes only for `web-wasm`. `imageNoise` keeps boxes with a count. Noise sets `names` and `png`, and the crops are written as `image-noise-<rect>`. `names: []` does not break `namedReport` or the other readers: the report already treats a missing list as `[]`, and an empty list only makes the crop call run so the noise files are written.

The record's noise line gives each box its rect, count, max level, and both crop filenames. `scripts/stage12-gate.mjs` is 1500 lines by `split('\n').length`, the count the line cap uses, so it is not over. The tests cover both sides of each rule, and the stage12 named and gate tests passed (41 pass, 0 fail).

VERDICT: CLEAN
