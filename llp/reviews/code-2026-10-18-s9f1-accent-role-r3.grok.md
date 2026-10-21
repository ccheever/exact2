# Code review: LLP 1121 stage 9 F1, AccentColor under an authored accent on the painters and iOS, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and rounds 1-2.
- **Method:** one brief (sha256 `4521b0e9a8a049fb99d9fcf010797d22cce4f5c1a0286c0f3cacf403d78f3079`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (no findings).
- **Disposition:** fixed in round 3's commit.

---

Nothing is blocking. No findings.

Round 1's iOS fixes hold, round 2's `authorsAccent` and `ownTint` fixes hold, and recording the two Linux cases on the existing painting-scope line is the right call.

**Round 1.** `authoredAbove` (`ProgressIOS.swift:83`) starts at `listenerParent` (`OwnersIOS.swift:75`). A dialog jumps through `sheets.authoredParent` to the superview saved at presentation, a lifted popover through `menus.logicalParent` to the superview saved when it was lifted, and a route controller inside a modal through `eventParent` to the tab panel or routes owner. Children walk superviews until that root, so an accent authored on a real ancestor inside the sheet, popover, or route is still seen. The kept colour is resolved with the spinner's dark, contrast, and elevated traits (`ProgressIOS.swift:54-55`), and `applyAccentTint` runs in the style pass before `syncProgress`. The role test is the wire name, not a string dump.

**Round 2.** `authorsAccent` (`ProgressIOS.swift:74-78`) treats only `sys == "@tint"` as the role. `systemRed`, a label colour, a `named:` asset, a channel pair, and Highlight's `@tint/0.2` all count as authored, on the spinner itself and on an ancestor. Round 1's `hasPrefix("@tint/")` returned false for every other `sys` and never reached `!namesTint`. LinkText, ButtonText, `link`, and `symbol` also cross as bare `@tint` on iOS, so the walk continues; on iOS those roles are the tint, and a further solid accent is already that view's `tintColor`. `ownTint(for:)` is nil unless the row names the tint, so a solid or system colour still resolves with the spinner's traits only. For `@tint/0.2` it passes the authoring view's `tintColor`, and `SystemColor.channels` multiplies that by the alpha (`SystemColor.swift:85-88`). That view's own `applyAccentTint` leaves `tintColor` unset when the row names the tint, so the colour is the inherited tint at 20%.

**Linux in QUEUE.** Acceptable. Both cases are the painting scope already tracked at `QUEUE.md:179`, which is explicitly not a few lines. The in-walk spinner is fixed: `node` opens `accent_scope` (`paint.rs:966`) before `control::authored_accent` (`control.rs:153-167`) reads `scoped_accent`. A recorded row is item (7): `row` (`paint/rows.rs:512`) walks and records while the scope is open, so the first pixels are right, and a later change of only the outer accent leaves the computed value as the role, which `row_valid` does not compare. Folding the spinner into that item matches the mechanism. The ghost is item (8): `paint_ghost` (`paint/lift.rs:117`) runs after the root walk's scopes drop. It loses the accent only when the ghost root's computed accent is the AccentColor role; `accent_of` still sees a concrete colour inherited onto that root (`computed_source` in `defaults.rs:235`). Before this change the spinner never read the scope, so neither case is a regression.
