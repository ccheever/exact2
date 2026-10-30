# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck14-record-lists, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `0e2d4fa5d1875f7677d8246eeac1a8916e5b8523af9dc10bc88ae60888954e20`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of THREE commits, `git log -p d07d6991d9~2..HEAD -- scripts` (the review-records commit between them is docs), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
- 2a7959a93f drops the profile-menu `follow` name, at Grok's ask in /tmp/iosfix-rv-g15-brief.txt's re-check: those pixels are the dark tint of the drive's pressed more button, so a rerun, not the adoption.
- HEAD adds to consumer-pixels' record, for theme2's reading (her 10:47 condition (7)):
  - every named box's rect, pad included, and its two crop files, shot by shot;
  - the allowlist's rects shot by shot (the caret strips and the iOS keyboard band).
Check:
- that the listed crops are the files namedCrops writes (the name-folding matches);
- that the rect listed is the one the pixel diff used;
- that the allowlist lines are the comparison's own;
- the line cap;
- nothing else changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check those three commits on the s15b2 gate branch, read-only, against the brief and the pixel-record rules. First I'll read the re-check note and the script diff.The range is the follow-name drop, the review-records docs commit, and HEAD's pixel-record lines. Next I'll match those lines against how crops, rects, and the allowlist are actually produced.No findings.

`2a7959a93f` deletes only the profile-menu `follow` entry at `scripts/stage12-s15b2.mjs:76` (the line is gone; `author-bo`'s `follow` stays). The per-host tally in `scripts/stage12-named.test.mjs:182` still counts `sheet|groups` only, so that drop does not change it. `d07d6991d9` is review records only. `28932e8724` touches only the evidence list in `consumerPixels`.

- **Crops.** `namedCrops` writes `<folded shot>-<folded target>-before.png` and `-after.png` (`scripts/stage12-named.mjs:114`). The record at `scripts/stage12-gate.mjs:553` uses the same fold (`/[^\w.-]+/g` → `-`) and lists those basenames. Across the 14 names, 5 hosts, and both schemes (224 files), each line matches exactly its own before and after file, in that order.
- **Rect.** The listed `[x, y, w, h]` is `m.rect` on the names `pixelDiff` returns (`scripts/stage12-named.mjs:41`, used at `scripts/stage12-named.mjs:52`). That rect is the union of the two sides after `namedOf` has applied the pad (`scripts/stage12-named.mjs:85`, `scripts/stage12-gate.mjs:287`). It is not the tight differing-pixel box.
- **Allowlist.** `scripts/stage12-gate.mjs:554` prints `x.allow` from `comparePixels` (`scripts/stage12-gate.mjs:286`), the same entries whose rects are passed into `pixelDiff`. Shots that never compared have no `allow` and are omitted. The caret and keyboard reasons stay distinct after the first-comma cut.
- **Line cap.** `scripts/stage12-gate.mjs` is 1497 lines by the caps count (`split('\n').length`). The rule fails only above 1,500 (`rules/RULES.md:16`, `scripts/caps.mjs:165`).

VERDICT: CLEAN
