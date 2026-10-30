# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck9, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `5db310755e78b45b6dfa902e0577b5d588a8759fdc6afeea43b04473f545ed57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
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

No P1/P2/P3 findings.

All three fixes check out. HEAD changes only the stated path handling, provenance reuse, and build-output logging. Focused in-memory checks passed; no files changed.

VERDICT: CLEAN
