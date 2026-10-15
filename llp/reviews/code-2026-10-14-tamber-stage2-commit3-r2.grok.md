# Code review: LLP 1121 stage 2, commit 3, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `af9199d98092eb367879ae6d12c32971a36c51e4dbd14faf5d1c7c4b1fac9952`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 2's commit (72790081)

---

The set-bit fix still drops a boundary's own implied colour on the way back to unset. The other round-2 fixes hold.

`boundary_rows` now skips an inherited colour only when that binding actually set the row. It does not forget the implied text it wrote earlier. The next time the binding unsets, `want == implied[k]` and `force` is still false (`colour_moved` is only consumed later, in `enter`), so the rewrite is skipped. The binding has already queued `ClearStyle`, and that clear wins.

That shows up when the colour binding and the boundary are the same node: `color=(on ? "#ff0000" : none)` together with `-exact-mode` or `color-scheme`. The first unset paints the ancestor's token in this tuple. Turning the override on is fine. Turning it off again leaves the row cleared, so text inside the boundary inherits the tuple above it. A dim pin under a black ancestor keeps the black slice. `enter` still hands the stale implied token to descendants, so a nested boundary can repaint while this node and its plain children do not. The new test never hits this: the unset binding sits above the pin, which is the path `enter` handles.

Checked and sound:

- `colour_moved` does not walk the subtree on every update. `index` and `unthemed` run only when that binding's value changed or it was forced, and a forced reapply already means `retheme` or a new tuple. `enter` takes the flag, so it does not stick.
- The create path's clear is right. Children are realized after `enter`, fresh, and they read the inherited text `enter` just published. A parent's `retheme` is kept by `|=`.
- `around` and `around_moved` re-set a pin whose text did not change when the tuple around it did, including a nested pin. Folding `retheme` into `around_moved` only rewrites that same tuple number.
- `enter` passes an unset row's surrounding token through, which is the case the new test covers.
- Live rows only in `set_token`, plan-scoped edits, role refusal, the document fold's scheme check, duplicate computed pins, the eight inherited colour rows, the lint binding its own design's axes, and the digest move all match the commit.

VERDICT: fix (MAJOR runner/src/instance/theme.rs:272)
