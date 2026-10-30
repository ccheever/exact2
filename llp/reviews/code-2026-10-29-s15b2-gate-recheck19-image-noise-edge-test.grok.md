# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck19-image-noise-edge-test, 2026-10-29 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree of the branch's commit under review (`c75cbf62af`).
- **Method:** one brief (sha256 `0487f29261d4284dcd92ac967acb18eafdfce043e187a3c084670bb730c7d70c`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of the last commit, `git show HEAD` (test only, scripts/stage12-named.test.mjs) on the s15b2 gate branch: Astra's P3 from the previous re-check said the image rule's ordering test never exercised the web edge floor (a flat before image). The new case: an edge pixel inside an Image box, changed by 2 levels (within the floor of 2 at an edge), must be counted by the image rule (images[0].count 1, max 2) and not left to the edge floor; without boxes the floor takes it (differ 0). Check it would fail if the edge floor ran before the image rule, and that nothing else changed (HEAD~1 is a QUEUE.md line: the wasm raster nondeterminism, theme2's condition (4); check its numbers read as the commit message of HEAD~2 states them). Read-only: no checkout, restore, stash, reset or edits.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

`c75cbf62af` on `x2/s15b2-gate` adds six lines in `scripts/stage12-named.test.mjs` and nothing else. The worktree is clean. `HEAD~1` (`fb2a84d175`) is the one `QUEUE.md` line.

The new case is an edge pixel the floor would otherwise swallow. `edgy` splits at `x < 4` (0 against 200). The only change is `(3, 4)`, from `[0,0,0,255]` to `[2,2,2,255]`: one pixel, 2 levels, inside `[2, 2, 4, 4]`, and a 4-neighbour 200 apart, which is past `edge` 32. That is the web floor (`WEB_NOISE` is `{ floor: 2, edge: 32 }`). `scripts/stage12-named.mjs:57-59` counts an image box and then applies the floor. With the box, `pixelDiff` returns differ 0, `images[0].count` 1, `images[0].max` 2 (`scripts/stage12-named.test.mjs:244`). With no boxes the floor takes it, differ 0 (`:245`). With neither floor nor boxes that same pixel differs by 1, so the second expect is the floor.

Replaying the same pixels with the floor checked first leaves the box at count 0 and max 0, so the expect of `[0, 1, 2]` fails. The older order assertion above it still uses a flat before-image and a change of 3, which is past the floor of 2, so it still does not exercise this.

The raster counts in `QUEUE.md:3` that a commit message states are in `6ab243fa77` (`HEAD~3`), under theme2 22:01 (1): light chat 286 px at most 2 levels between two drives, dark chat 288 px at most 4 against the gate's run, ceiling 4. The queue line repeats those. `HEAD~2` (`3268b536`) states the 1500-line cap and the rule order, and this commit does not touch that. The queue line also gives new-chat as 243 px, the avatars as 42 and 52 pt, and an identical second pair. Those three are not in either message. `6ab243fa77` also says dark chat went from 0 to 70 px between runs.

The image-raster test passes (1 pass, 0 fail).

VERDICT: CLEAN
