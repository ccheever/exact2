# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck9, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `5db310755e78b45b6dfa902e0577b5d588a8759fdc6afeea43b04473f545ed57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
The previous re-check (/tmp/iosfix-rv-g10-brief.txt, of the speedup commit HEAD~1) found three things:
- Astra P2: a relative EXACT2 was resolved against the gate's cwd, not the copy;
- Grok P2: a relative --copy made EXACT_APP_DIR resolve twice;
- Astra P3: the web build's warnings were no longer kept.
This commit:
- sdkOf resolves an EXACT2 override against the copy (as the copy's launcher does), and provenance now uses sdkOf;
- EXACT_APP_DIR is resolve(copy);
- buildSide writes each build's whole output to target/stage12-gate/build-<side>-<verb>.log.
Check these, and that nothing else changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

The commit is one file, seven lines. I'll check those three fixes against the launcher and the build output path, and confirm the rest of the diff is unchanged.No P1, P2, or P3 findings. The three fixes hold, and the diff is those edits in `scripts/stage12-gate.mjs` (7 lines).

- A relative `EXACT2` is resolved against the copy in `sdkOf` (`scripts/stage12-gate.mjs:409`), the same `resolve(directory, EXACT2 ?? default)` the copy's launcher uses. Provenance takes that path (`scripts/stage12-gate.mjs:241`). An absolute override still stays absolute.
- A web drive sets `EXACT_APP_DIR` to `resolve(copy)` (`scripts/stage12-gate.mjs:191`). The agent's later `resolve` keeps that absolute path, so a relative `--copy` or `--before` stays the copy directory for `consumerPixels`, baseline, retries, and keyboard redrives.
- `buildSide` writes each build's full stdout and stderr, compiler warnings included, to `target/stage12-gate/build-<side>-<verb>.log` (`scripts/stage12-gate.mjs:455-456`) before it treats the build as failed. The JS `web-build` prints those warnings.

VERDICT: CLEAN
