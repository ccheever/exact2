# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the press-release commit.
- **Method:** one brief (sha256 `3a2ed2e7889aea630e388520a40744e86e4ce0a61fc5bc0e9d519ee7e1e7115e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (the self mark lost the text lender; labels; a timing-dependent test).
- **Disposition:** fixed in round 1's commit.

---

Nothing blocking. Three should-fix findings from static review; no builds or tests were run.

1. **Should-fix — marking the whole view self-owned loses the text lender.** [PressPaint.swift:192](/tmp/rv-g4-1/host/apple/Sources/ExactKit/PressPaint.swift:192), [PressPaint.swift:243](/tmp/rv-g4-1/host/apple/Sources/ExactKit/PressPaint.swift:243). A lends yellow over a white base; B starts a background-only press. If A changes its press colour to green, `inkBelow` now skips B: its owner is B and yellow differs from white. Cleanup restores B’s text to white instead of updating it to green.

   The three-level case also fails: A initially lends white, B re-inks its descendants green, then C starts a background-only press. A’s delayed release clears C’s green through the self-owned fallback, although that colour belongs to B’s still-held press. **Fix:** retain the text lender separately from ownership of the background; use that provenance in `inkBelow`, `endInk`, and `endInked`.

2. **Should-fix — the fix leaves already-inked text children outside the descendant’s own press.** [PressPaint.swift:194](/tmp/rv-g4-1/host/apple/Sources/ExactKit/PressPaint.swift:194). With white base text, A lends yellow to B and B’s label C. B presses with `press_color: green`, but its walk compares C’s displayed yellow against B’s saved white and skips C. When A releases, B stays green while C becomes white despite B remaining down. This inheritance gap already exists on press-down in the parent revision; HEAD preserves B without fixing its label. **Fix:** recognize inherited colour through the saved base/previous lender and transfer eligible children to B. Add a nested-label case. B’s own `rest["text_color"]` correctly retains the original white.

3. **Should-fix — the regression test can pass against the old code.** [PressPaintTests.swift:131](/tmp/rv-g4-1/host/apple/tests/ExactKitTests/PressPaintTests.swift:131). If painting and the assertion before release take longer than `PressFeedback.unseen` (16.7 ms), A releases immediately, before B presses. Every subsequent assertion then passes even without this commit. The fixed run-loop wait does not establish that the delayed path occurred. **Fix:** test overlapping paint deterministically using `paintPress(..., instantly: true)` and keep timer coverage in the quick-tap test; alternatively, assert `releaseHeld` and wait for release completion rather than a fixed duration.
