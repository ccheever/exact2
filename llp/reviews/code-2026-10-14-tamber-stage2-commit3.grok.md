# Code review: LLP 1121 stage 2, commit 3 (app axes, the slice and mode boundaries), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `126302baf4bfd50bc436c42a0bf3506f602055c16c5282ccb837297c11861b14`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 1's commit (416b54839); the virtualized-list rekey cost (Grok P2) noted, not changed

---

## Findings

### [P1] `set_token` styles destroyed views, then keeps the edit
`runner/src/runner/theme.rs:36`

The theme index drops a view only from `Ids::retain`, and that runs only after remembered views exceed twice the live count plus 256 (`runner/src/runner.rs:1439`). Scrolling a virtualized row off, shrinking a `for`, or switching a `when` arm destroys the view and leaves its `(view, style)` keys in the index. `set_token` emits `SetStyle` for those ids. The kernel rejects the whole batch (`UnknownView`), so the live rows stay as they were. `edit_token` has already swapped the thread-local table, so a later parse of those same rows uses the new value while the screen still shows the old one.

### [P1] A partial pin leaves `-exact-mode` on the old tuple
`runner/src/instance.rs:973`

`forced` is `force && themed`. The mode binding reads no token, so an inherited axis moving under an unchanged pin text does not re-emit it. `self.tuple` and the themed colour rows take the new mixed-radix tuple. The kernel row stays at the previous `tuple + 1`. A child pinned only to `dark-style=dim` under a parent whose `density` pin flips is that case: the colours follow the child, and the inherited mode row still names the previous tuple.

### [P1] A boundary keeps the previous inherited token text
`runner/src/instance/theme.rs:177`

`retheme` is set only when this node's tuple changes, and `update_all` skips a child whose own reads are fresh. A page `color={name}` above a `color-scheme` or `-exact-mode` card updates the page and leaves the card's implied colour on the old text. That implied row is specified, so kernel inheritance does not replace it. `set_token` refreshes a boundary that already recorded that same text. It does not retarget the boundary when the ancestor binding starts naming a different token.

### [P1] An unset colour binding counts as setting the row
`runner/src/instance/theme.rs:232`

`boundary_rows` skips every inherited colour that has a binding, including one that just cleared the row (`none`, or a value the row refused). `enter` then publishes `None` for that channel (`theme.rs:170`) because the themed bit is clear, so a boundary under it also stops re-specifying the ancestor token. A preview pin with `color={cond ? "ink" : none}` shows the outer slice while `cond` is false.

### [P2] A tuple change rekeys every virtualized item
`runner/src/instance.rs:548`

`stale_outside` is true for every read while `retheme` is set, and the collection treats that as `keys_stale`. A pin change above a virtualized list re-evaluates every item key. `changed_outside` stays false, so measurements and the revision hold when the keys compare equal. The work is still the whole list, not the mounted rows.

### [P2] The lint checks pins against another compile's axes
`contract/lower/src/theme.rs:531`

`mode_pins` reads the thread-local axis table that `declare_theme` fills. The lint runs when typechecking or analysis fails, before `declare_theme`. A file with both a type error and `-exact-mode` is checked against an empty table or the previous file's axes, so the extra diagnostic names the wrong axes or claims the design has none.

## What held

Mixed radix matches the crate (first axis slowest). An invalid pin unsets the row and `tuple_of` falls back to the surrounding tuple. The mode binding is parsed first, over that surrounding tuple. A pin whose text changes does re-walk the subtree (`retheme`), which is what `a_changed_pin_sets_its_subtrees_themed_rows_again` covers. Virtualized rows mounted later, and rows rebound on reuse, parse against the list's saved `around`. The document fold refuses a plan with axes, and the direct render falls back to a kernel. The schema digest moved to `0xd9f60a81ec13304d`. Web and the JS target emit nothing for the mode row. Linux, terminal, and Apple take the new field from the generated schema. Apps with no theme skip the index, the boundary pass, and the mode lookup. `theme_alloc` still passes. `set_token` ors the role spelling into the bloom and merges `written` through the existing style patch. The runner choosing the slice, and `axis=value` where D5 still shows `dark-style:dim`, match this commit's schema and tests. `tamber-inherited-axis` is still deferred.

`cargo test -p contract --test it tamber::` — 18 passed. `cargo test -p contract --test theme_alloc` — passed. Nothing covers a partial pin under a moving inherited axis, a changed inherited token above a boundary, `set_token` after a themed view is destroyed, an unset colour on a boundary, or the document-fold refusal.

VERDICT: fix (MAJOR `runner/src/runner/theme.rs:36`, `runner/src/instance.rs:973`, `runner/src/instance/theme.rs:177`, `runner/src/instance/theme.rs:232`; MINOR `runner/src/instance.rs:548`, `contract/lower/src/theme.rs:531`)
