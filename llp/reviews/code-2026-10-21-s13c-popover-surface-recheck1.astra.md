# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), narrow re-check after round 3 (theme2 00:54), 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b7f9b81af1`.
- **Method:** one brief (sha256 `9f28b459a4a36f46d9762cd627c8bd0c37f5c34b940e27500d39c5ca331fb93d`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** both should-fixes taken (unwritten corners are 0; percentages get no surface); a second narrow re-check follows

VERDICT: WRONG

1. **Should-fix — [Backdrop.swift:191](/tmp/iosfix-rv-ps4/host/apple/Sources/ExactKit/Backdrop.swift:191): Explicit zero can incorrectly remove the glass.** With only `border-top-left-radius=0`, the guard compares zero against three `.null` values and reports uneven corners. The renderer resolves all four corners to zero, so this is a supported square surface that previously worked. Normalize omitted corners to their zero default when comparing.

2. **Should-fix — [Backdrop.swift:180](/tmp/iosfix-rv-ps4/host/apple/Sources/ExactKit/Backdrop.swift:180): Equal percentage radii produce incorrect scroll clipping.** A 200×100 popover with `border-radius="10%"` passes `unevenCorners`, but its corners resolve to 20×10 ellipses. The new helper uses only the width, installing a 20-point circular clip. This clips children differently from the authored shape; calling it after layout does not fix that mismatch. Check resolved circularity or preserve the resolved outline.

The per-batch removal and compiler exclusions look correct. Neither accepted limit is reopened here. Read-only source review; builds and tests were not run.
