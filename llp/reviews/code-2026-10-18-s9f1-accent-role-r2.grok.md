# Code review: LLP 1121 stage 9 F1, AccentColor under an authored accent on the painters and iOS, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `bc62c627b6cd29d9e2b114bccbe4945e0fa8fd366337c8dd5fcbfbccebe8c89c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (system colours taken as the role; the kept accent without its view's tint; a QUEUE numbering nit). (Astra's round 2 did not run: the model was at capacity.)
- **Disposition:** fixed in round 2's commit.

---

Nothing is blocking. The literal-accent case, the authored-tree walk, and resolving with the spinner's appearance are in place. Recording the two Linux cases on the existing painting-scope entry is acceptable.

1. **should-fix** — `host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:77`
   `authorsAccent` returns true only when a system-colour object's `sys` starts with `@tint/`. Every other `sys` (`systemRedColor`, `labelColor`, `named:Brand`) is treated as the bare tint role. A column with `accent-color="-exact-system-red"` around a spinner with `accent-color="AccentColor"` is skipped, and the spinner falls through to `ThemeRoles.color("AccentColor")`. Linux keeps that role in the painting scope and paints system red. A spinner that itself sets `accent-color` to a system colour or a `named:` asset does the same: `written` is false, so its own colour is never read. Before this round the string check treated any value that did not contain `@tint` as authored.
   **Fix:** in the `sys` branch, return `sys != "@tint"`. `@tint/0.2` stays authored; `systemRedColor`, `labelColor`, and `named:` stay authored; only the bare `@tint` role does not. Keep the `!namesTint` fallthrough for arrays and wide colours.

2. **should-fix** — `host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:55`
   The kept colour is resolved with the spinner's dark, contrast, and elevated traits, and without a tint. A page with `accent-color="#00c800"` sets that view's `tintColor` to green. A child with `accent-color="Highlight"` (`sys` `@tint/0.2`) is the nearest authored accent, so the walk stops there. `channels` then takes `SystemColor.tint`, the key window's app tint, at 0.2 alpha. A label with `color="Highlight"` in that child uses `ownTint` and paints green at 0.2. The spinner paints the key window's tint at 0.2. In a non-key window the same call reads the key window. The previous round called `NodeView.channels`, which passes `ownTint`.
   **Fix:** pass `tint: owner.ownTint(for: row)` on that `channels` call, next to the spinner's dark, contrast, and elevated values.

3. **nit** — `QUEUE.md:179`
   The painting-scope entry now has two items numbered (7). The new retained-row sentence restates the existing recorded-row item (inheritance stops at the `accent-color` declaration in `txn.rs`; `paint/rows.rs` does not compare the effective accent). The new fact is that stage 9 F1's spinner reads the scope too.
   **Fix:** add that clause to the existing (7), and leave the lifted ghost as (8).

Queuing those two instead of fixing them here matches that entry. The scope is thread-local and filled only by a full paint walk, and a role declaration stops inheritance, so a retained recording stays on the accent it was recorded with and a ghost painted from the lifted row collapses `accent-color: AccentColor` to nothing when the concrete accent is outside the row. Both are the class already listed there (the walk-only scope, and the recording that does not track the effective accent), and each is more than a local spinner change.
