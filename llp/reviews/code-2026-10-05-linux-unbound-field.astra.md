# Round 1
Found three defects:

1. **P1 — Hardware edits can remain invisible.** [typing.rs:508](/Users/admin/projects/exact2-wt-unbound/host/linux/src/presenter/typing.rs:508) stores the edit without marking the presenter dirty. For an already-focused `input change=save` with no `input` or `key` handler, typing updates `chosen`, but the display loop skips repainting. Backspace and textarea Enter have the same problem. **Fix:** mark accepted text edits dirty independently of event handlers.

2. **P2 — Old typed text resurrects after a bound-value round trip.** [typing.rs:549](/Users/admin/projects/exact2-wt-unbound/host/linux/src/presenter/typing.rs:549) only checks whether the current bound value equals the saved baseline; it never expires the entry. Type `"draft"` against bound `"a"`, then change the binding to `"b"` and back to `"a"` without editing: Linux shows `"draft"` again, whereas the web shows `"a"`. Removed nodes’ entries also remain indefinitely. **Fix:** invalidate retained entries when their bound value changes or their node disappears, and prevent `keep_typed` from inserting an already-removed node.

3. **P2 — The tree overlay reports unsanitized range values.** [agent.rs:649](/Users/admin/projects/exact2-wt-unbound/host/linux/src/agent.rs:649) applies `field_text` to every `chosen` entry, including ranges. For a range bounded at 100, typing `101` can store `"101"` in `chosen` while the action receives—and the painter shows—100. The new overlay then reports `"101"`, replacing the previously correct bound value. **Fix:** restrict this overlay to text fields, or resolve each control’s displayed value using its sanitization rules.

No additional defects found in the requested text replacement, maxlength, textarea, masking, placeholder, literal-value, or ordinary focus paths. The Linux search found no other text-value reader needing conversion.

All 15 presenter event tests and the relevant controls test passed using the existing compiled test binary. No files changed.
# Round 2
Two defects remain:

- **[P2] Batched commits can still resurrect typed text** — [host/linux/src/presenter.rs:668](/Users/admin/projects/exact2-wt-unbound/host/linux/src/presenter.rs:668). `set_focus` dispatches `change`, `blur`, and `focus` before sweeping. If a field holds `(typed, "A")`, its `change` handler sets the bound value to `"B"`, and its `blur` handler restores `"A"`, the sweep retains the stale text. Invalidation must observe each committed value change, not only the final value.

- **[P2] Typed text can transfer to an unrelated field after reload** — [host/linux/src/presenter.rs:471](/Users/admin/projects/exact2-wt-unbound/host/linux/src/presenter.rs:471). Reload replaces the runner without clearing `chosen`; delivery activation does likewise. New runners reuse numeric view IDs. A different unbound input receiving the old ID also has bound value `""`, so the sweep preserves—and paint/commit consumes—the previous field’s text. Clear retained entries when replacing the tree.

The dirty flag and TextInput-only overlay fixes look correct. Ordinary setter ordering does not incorrectly discard valid choices. Three focused tests passed using the existing test binary; no files changed.
# Round 3
Three defects remain:

1. **Batched receipts still lose intermediate values** — [host.rs:1217](/Users/admin/projects/exact2-wt-unbound/host/linux/src/host.rs:1217). `advance_timed` and `fulfill_all` apply multiple commits before `commit_effects` processes their receipts. Each iteration reads the **final** kernel value. If two timer firings change `"A" → "B" → "A"`, both records contain `"A"`, so typed text survives incorrectly. Values must be captured when each commit occurs.

2. **Insertion can resurrect an edit after its invalidation was drained** — [typing.rs:576](/Users/admin/projects/exact2-wt-unbound/host/linux/src/presenter/typing.rs:576). For example, an input handler changes the bound value from `"A"` to `"B"` and changes a box’s width; its synchronous `resize` handler restores `"A"` during `after_commit`. Both commits are recorded and drained, but afterward `keep_typed` sees `bound == before` and inserts the typed text again. The pending edit needs to participate in invalidation before those callbacks run.

3. **Standalone `Host` instances accumulate history indefinitely** — [host.rs:1224](/Users/admin/projects/exact2-wt-unbound/host/linux/src/host.rs:1224). Every touched control appends an owned value, and every destroyed view appends a tombstone. Only `Presenter` drains this vector; `take_bound_values` is crate-private. A caller using public `Host::boot` and repeatedly dispatching or advancing cannot drain it, producing unbounded memory growth.

The replacement clear covers both reload and delivery activation. Ran 16 relevant tests from existing binaries; all passed. No files changed.
# Disposition (after round 3)

Fixed in the last commit: the host now watches only the values a typed text
or choice is kept against (`value_watch.rs`), set before the edit's dispatch,
so a standalone `Host` grows nothing and a write during the edit (or its
`after_commit` callbacks) replaces the typed text even if restored. Left, and
noted in LLP 1069.001: receipts handed over in one batch (timers in one clock
advance) are read at the batch's end, so A to B to A inside one batch is not
seen; and `field-sizing: content` measures the prop.
