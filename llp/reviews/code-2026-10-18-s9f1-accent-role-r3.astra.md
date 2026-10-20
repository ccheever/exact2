# Code review: LLP 1121 stage 9 F1, AccentColor under an authored accent on the painters and iOS, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change and rounds 1-2.
- **Method:** one brief (sha256 `4521b0e9a8a049fb99d9fcf010797d22cce4f5c1a0286c0f3cacf403d78f3079`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (pushed routes and carried rows outside the walk; test coverage).
- **Disposition:** fixed in round 3's commit.

---

Nothing is blocking.

1. **should-fix — Authored ancestry still skips tab panels and carried rows.** [ProgressIOS.swift:84](/tmp/rv-f1-3/host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:84) uses `listenerParent`, which redirects navigation routes only inside modal presentations. With a red root, green tab panel, and a nonmodal route declaring `accent-color="AccentColor"`, the spinner’s next sync walks UIKit’s containers past the panel and selects red. Carried grouped rows similarly bypass their authored sections. **Fix:** include `navigation.logicalParent` for all owned routes and `groupedLists.logicalParent`, preserving the sheet/popover links. Add regression tests after native projection.

2. **nit — The review fixes lack regression coverage.** [ProgressIOSTests.swift:62](/tmp/rv-f1-3/host/apple/tests/ExactKitTests/ProgressIOSTests.swift:62) tests fixed green through an ordinary view hierarchy. Reverting the ancestry, appearance, system-colour classification, or `ownTint` fixes would leave it passing. **Fix:** cover presented content, differing ancestor/consumer appearances, `systemRedColor`, and `Highlight` under a nondefault view tint.

The bare-`@tint` comparison and the explicit owner-traits/authoring-view-tint arguments address the reported cases correctly.

Deferring the Linux cases in [QUEUE.md:179](/tmp/rv-f1-3/QUEUE.md:179) is reasonable for this patch: recording invalidation was already item (7), and the ghost extends item (5)’s existing scope limitation. They remain unfixed; the shared repair needs effective-accent propagation into detached painting and recording invalidation when that accent changes.

Read-only review of all three commits; no builds or tests run.
