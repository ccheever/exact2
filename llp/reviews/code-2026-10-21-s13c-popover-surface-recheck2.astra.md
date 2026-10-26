# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), second narrow re-check, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d16dbf1d10`.
- **Method:** one brief (sha256 `88585f5ad2cfc43e014ec3753fe4bccd723ab4a53865e3420cc9e77e900b021c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (two P2s)
- **Disposition:** both taken in 6e615a7891 (any corner that is not points counts as uneven; the test uses the wire's {"pct":10} rows on a scrolling popover); a third narrow re-check follows

VERDICT: WRONG

1. **P2 — Real percentage radii still bypass the guard.** [Backdrop.swift:194](/tmp/iosfix-rv-ps5/host/apple/Sources/ExactKit/Backdrop.swift:194). `border-radius="10%"` becomes four corner rows, each serialized as `{"pct":10}` by [style.rs:503](/tmp/iosfix-rv-ps5/host/apple/src/style.rs:503). It reaches NodeView as `.object(["pct": .number(10)])`, whose key is `{"pct":10.0}`—no `%`. All four keys match, so `unevenCorners` remains false. Glass survives, and a 200×100 scrolling popover still receives a circular 20-point clip instead of respecting its 20×10 elliptical radii. Deferred `.calculation` keys likewise contain numeric handles and environments, without `%`. The check needs typed percentage/geometry information.

2. **P2 — The added percentage test misses the production representation.** [PopoverSurfaceIOSTests.swift:198](/tmp/iosfix-rv-ps5/host/apple/tests/ExactKitTests/PopoverSurfaceIOSTests.swift:198) injects `"10%"`, which decodes as `.string`. [BatchValue.length](/tmp/iosfix-rv-ps5/host/apple/Sources/ExactCore/Batch.swift:36) does not parse percentage strings, so this fixture never represents elliptical corners. Use the four `{"pct":10}` corner values or compiled Contract input, and assert that a scrolling popover receives neither default glass nor its circular clip.

The `.number(0)` fallback correctly fixes the single-zero corner case. No additional regressions identified. Static review only; tests not run.
