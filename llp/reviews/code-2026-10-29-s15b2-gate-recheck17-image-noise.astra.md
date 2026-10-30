# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck17-image-noise, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review (`6ab243fa77`).
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

- **P2 — [scripts/stage12-gate.mjs:557](/tmp/iosfix2-rv-wn-astra/scripts/stage12-gate.mjs:557): Noise crop paths are missing from the record.** The evidence prints rect/count/max and the pattern `image-noise-<rect>`, but neither actual crop filename. The preceding filename list visits only `x.names`, excluding image noise. List both crop paths per noise box.

- **P2 — [scripts/stage12-gate.mjs:519](/tmp/iosfix2-rv-wn-astra/scripts/stage12-gate.mjs:519): Partially visible images produce incorrect crops.** Image rects reach `namedCrops` without clipping to screenshot bounds. Reproduced with `[1,-1,2,3]`: the crop’s first row comes from the screenshot’s **last** row because `crop()` uses negative `subarray` offsets. Intersect noise crop rects with screenshot bounds.

- **P3 — [scripts/stage12-named.test.mjs:212](/tmp/iosfix2-rv-wn-astra/scripts/stage12-named.test.mjs:212): Coverage does not exercise every requested rule.** The new test covers thresholds and matching/moved geometry, but omits image interactions with allowlists, named boxes and edge noise; host restriction through `comparePixels`; duplicates; and noise-only crop/report behavior.

Ten selected tests passed. In-memory probes confirmed rule ordering, host restriction, duplicate handling and safe `names: []` behavior. The gate is exactly 1,500 lines. No files changed.

VERDICT: CHANGES
