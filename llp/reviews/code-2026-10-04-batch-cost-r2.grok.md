# Code review, round 2: iOS host batch cost (bbe96611b), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `bbe96611b`.
- **Method:** one brief (sha256 `1cce0a4c559f7ed242bf61d354404d2fa43b7c10303b2ad8f543deab87e708bf`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r3):** 1 (stale faces when an edit emits no op) is taken as suggested: a batch flag, not an empty props op. The test now drives the flag rather than an unrelated op.

- **Superseded (2026-10-04):** the branch these reviews were of was abandoned after round 3 and not landed. See `code-2026-10-04-batch-cost-r3.*` and `code-2026-10-04-idle-tick.*`.

---

1. **Blocker** — A button face or a select menu stays stale when the edit emits no op. `PresenterIOS.swift:890`, `paragraph.rs:273`, `paragraph.rs:327`, `layout.rs:1187`.

`update` returns immediately for a button's viewless children and for an `option` (`option_part`). `emit_children` returns immediately for a `Control`, so a `SetProp` of the title text, the symbol source, or an option's label, value, or disabled flag writes nothing into the batch. Those nodes are not laid out either: a `Control` is a Taffy leaf, and its children are never attached (`layout.rs:1186-1188`), so the control's frame does not move and no follow-up frame op appears. `commit_tree` still applies that empty batch. The new skip treats it like a timer tick and does not call `controls.sync`. The `UIButton` keeps the old title, symbol, and accessibility label, and the pop-up keeps the old menu, until some later unrelated op happens to make a non-empty batch. Sync is also the only place that publishes a control's natural size (`ControlsIOS.swift:177-189`), so a shrink-wrapped button never reports the new face and the kernel never sends a repairing frame. `testAnEmptyBatchConfiguresNoControl` (`NativeButtonsIOSTests.swift:68-81`) changes the face table and then pokes an unrelated `props` op on the parent. That is the case the skip still configures, and the empty-batch assertion locks in the stale title.

**Fix:** In the Apple host, when a touched, created, or destroyed id is a button's viewless child or a select option, mark that batch as needing a control sync (a flag beside `timers`, not an empty `props` op — `applyProps` has side effects). In `Presenter.apply`, sync when that flag is set, the same way as `controlsStale`. Keep skipping a truly empty timer tick. Extend the test with a kernel batch whose only change is the button's text prop or an option's label, with no props, style, children, or frame op, and assert the `UIButton` title and the menu update.

Checked, and not defects of this change: keyboard and safe-area updates go out as resize and inset batches that carry frame ops. `glassGroups.reconcile` moves a glass button itself, after sync, and an empty batch does not change `controlMount` without that move. Grouped-list and swipe recycling reparent the `NodeView`, and the control is its subview. `segments.sync` and `navigation.sync` read kernel faces, not `ControlHost`, and they still run on an empty batch. Restart's next batch is a create batch. Agent activation on a native button presses the node. Snapshot-only batches already skipped control sync before this change, and they leave `controlsStale` set.

Verdict: DO NOT LAND
