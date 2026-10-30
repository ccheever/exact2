# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck19-image-noise-edge-test, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review (`c75cbf62af`).
- **Method:** one brief (sha256 `0487f29261d4284dcd92ac967acb18eafdfce043e187a3c084670bb730c7d70c`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of the last commit, `git show HEAD` (test only, scripts/stage12-named.test.mjs) on the s15b2 gate branch: Astra's P3 from the previous re-check said the image rule's ordering test never exercised the web edge floor (a flat before image). The new case: an edge pixel inside an Image box, changed by 2 levels (within the floor of 2 at an edge), must be counted by the image rule (images[0].count 1, max 2) and not left to the edge floor; without boxes the floor takes it (differ 0). Check it would fail if the edge floor ran before the image rule, and that nothing else changed (HEAD~1 is a QUEUE.md line: the wasm raster nondeterminism, theme2's condition (4); check its numbers read as the commit message of HEAD~2 states them). Read-only: no checkout, restore, stash, reset or edits.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings.

- [scripts/stage12-named.test.mjs:244](/tmp/iosfix2-rv-wn3-astra/scripts/stage12-named.test.mjs:244): focused test passes. Moving the edge floor first in memory makes it fail: `[0,0,0]` instead of `[0,1,2]`. Without boxes, `differ` is 0.
- HEAD adds only six test lines; HEAD~1 adds only the QUEUE line. Working tree remains clean.
- [QUEUE.md:3](/tmp/iosfix2-rv-wn3-astra/QUEUE.md:3): 286 pixels at ≤2 and 288 at ≤4 match **HEAD~3**, not HEAD~2. Neither commit message states the 243-pixel new-chat count, so that number remains unverified.

VERDICT: CLEAN
