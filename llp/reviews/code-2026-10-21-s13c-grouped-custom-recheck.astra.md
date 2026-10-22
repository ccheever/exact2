# Code review: iOS grouped lists: a custom row's leading symbol and line as a standard cell's (Harmony), narrow re-check after round 3 (theme2 20:51), 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e0aabc7fa9`.
- **Method:** one brief (sha256 `34dc123398f0ca50c9b7243228428abd15fb6606803168f8f66d7d28f154cb01`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** nit 1 taken (the comment); nit 2 (dedicated tests of exchanged and offscreen marks) left: refreshLooks and mark are exercised by the dropped-row and moved-slot tests

VERDICT: **RIGHT WITH NITS**

1. **Nit — [GroupedCustomRowsIOS.swift:67](/tmp/iosfix-rv-gcr4/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:67):** Authored `object-fit: contain` is correctly excluded and tested. The new comment overclaims size authorship, though: image width/height provenance is still filtered by [style.rs:131](/tmp/iosfix-rv-gcr4/host/apple/src/style.rs:131). Qualify the comment; that size-only ambiguity predates these commits.

2. **Nit — [GroupedCustomRowsIOS.swift:45](/tmp/iosfix-rv-gcr4/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:45):** Ownership transfer now handles exchanged symbols independently of traversal order. The mount-time `refreshLooks()` also clears marks from images leaving tracked offscreen rows. Both fixes look correct, but neither scenario has a dedicated regression test.

3. **Info — [GroupedCustomRowsIOS.swift:92](/tmp/iosfix-rv-gcr4/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:92):** RTL returns `nil`, preserving UIKit’s separator as ruled. The revised test checks that fallback; the asymmetric-width calculation is gone.

4. **Info — [GroupedCustomRowsIOS.swift:73](/tmp/iosfix-rv-gcr4/host/apple/Sources/ExactGroupedLists/GroupedCustomRowsIOS.swift:73):** Accessibility categories suppress the mark and restore existing symbol drawing. [Tests:134](/tmp/iosfix-rv-gcr4/host/apple/tests/ExactKitTests/GroupedCustomRowsIOSTests.swift:134) cover disabling it and restoring it at XXXL. [QUEUE.md:3](/tmp/iosfix-rv-gcr4/QUEUE.md:3) and [agent-pitfalls.md:511](/tmp/iosfix-rv-gcr4/docs/agent-pitfalls.md:511) correctly state the limits and deferred ramps.

No blocking finding in the requested scope. Static review only; no files changed or tests run.
