# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck15-island, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review (`53aed0056d`).
- **Method:** one brief (sha256 `fd01bdb577cd87bcdb2f7017ad8daaafc0e9af90e0a1cf4623c9719601601685`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, `git show HEAD` (53aed0056d) on the s15b2 gate branch (x2/s15b2-gate), touching scripts/stage12-gate.mjs, a new scripts/stage12-island.mjs and scripts/stage12-named.test.mjs. Read-only: no checkout, restore, stash, reset or edits.

Context: the gate compares before/after screenshots of the Bluesky clone copy; differing pixels outside named boxes and the allowlist fail. The simulator's capture draws the Dynamic Island's sensor housing (and the top-left corner pixels) in some iOS shots and not others (seen in 4 shots over two full runs, 4,490 px each at y13-50 x138-263 plus 3-4 px at x0-1 y0-1). theme2 ruled (15:32 (a)): harden it as an iOS-only allowlist of the island's fixed rect, allowed on a shot only when no library component's box intersects it (from the layout, like allowOf), listed per shot in the record.

Check:
- islandAllow returns entries only for host 'ios', only for rects no library node's box meets (a node is a library one by its sourceMap.file in packages/ or node_modules/@exact/, as renderedComponents reads it); touching edges don't count; zero-size nodes don't count;
- the entries reach the comparison (comparePixels unions before's and after's allow) and the record's per-shot allowlist lines, and a drive with no layout or a null allowOf still behaves as before (allow null => the shot fails as before);
- whether including the 2x2 top-left corner beside the island's rect is justified by the same capture toggle (it differs only in the shots where the island does); whether the rect is right for a 402x874 shot at a pixel a point;
- the coordinates: layout nodes are in screen points from the top of the 402x874 viewport (the named boxes use the same);
- the test covers both branches; the 1500-line cap on stage12-gate.mjs;
- nothing else changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings.

Confirmed iOS-only filtering, library-box intersections, edge/zero-size handling, preserved null failures, comparison union, and per-shot recording. Saved captures support both rectangles and the 402×874 screen-point coordinates.

The test covers allowed/blocked branches and passes; additional read-only checks pass. `stage12-gate.mjs` is 1,498 lines. Only the three specified files changed.

VERDICT: CLEAN
