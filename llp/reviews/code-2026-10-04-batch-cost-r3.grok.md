# Code review, round 3 (final): iOS host batch cost (0d118c2d7), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, a detached worktree at `0d118c2d7`.
- **Method:** one brief (sha256 `e6c8ae9d06d0d330e8aa67b38861e43068d72358f7405894e8c6547fc39f4650`), shared by both reviewers. Round 3, the last fix round, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition: superseded, not landed.** After three rounds the batch-cost branch (f3de06ddd..0d118c2d7) was abandoned on the coordinator's decision, 2026-10-04. Its idea was to skip work inside the presenter's pass. The empty batches came from the app-timer path, the one path without the session's existing "nothing changed" predicate (fills and list feedback already skip theirs). The replacement skips the whole pass there, and keeps r3's `controls` flag for the viewless-children case this round found. It is commit "Session: an idle timer tick only moves the clock", with its own reviews (`code-2026-10-04-idle-tick.*`). This round's blocker, that fast paths ignored `controls`, is addressed there: `changesNothing` and `applySnapshots` read the flag.

---

1. **Blocker** — A controls-only batch never reaches the presenter on the list-feedback path. `Session.swift:629`, `Session.swift:667`.

`Presenter.apply` syncs when `batch.controls` is set (`PresenterIOS.swift:889`). Two earlier gates still treat "no ops" as "nothing to apply", and neither reads `controls`. `landFill` (async fills, the iOS default at `Session.swift:438`) and `collections.onFeedback` both require a change in ops, error, motion, spatial, the timer deadline, or `canvasOwed`. A list edge action (`collection_feedback_filled` dispatches `reachstart` / `reachend`) that only retitles a native button or toggles an option's `disabled` produces `ops: []` and `"controls":true`. The Rust commit has already landed. The session drops the batch, `controls.sync` does not run, and a later identical scroll report is not sent again, so the `UIButton` title and the select menu stay stale. Press, timer, and `pump` call `apply` directly and are fine. The new tests never enter these gates: the Rust test only searches the JSON, and `testAnEmptyBatchConfiguresNoControl` sets `controls` on a batch it applies itself.

**Fix:** Treat `batch.controls` as a reason to apply in both predicates (`ops.isEmpty && !batch.controls && error == nil && …`). Add a host test whose only commit is an option or face change delivered through `collection_feedback`, and assert the presenter sees `"controls":true`.

2. **Should-fix** — A select's reported width ignores the control's content-size category. `ValueControlsIOS.swift:61`.

The per-control registration marks `controlsStale`, and the next sync does run. A native button then reads `control.intrinsicContentSize`, which follows that control's traits. A select measures a fresh `UIButton(configuration:)` that is not in the control's hierarchy (`ValueControlsIOS.swift:61-66`), so a descendant-only category override — the round-2 case — still sizes the pop-up to the default category and the kernel keeps the old frame.

**Fix:** Resolve the probe inside the live control's trait collection (`control.traitCollection.performAsCurrent`, or copy `preferredContentSizeCategory` onto the probe) before reading `intrinsicContentSize`.

3. **Should-fix** — Bold Text never marks controls stale. `ControlsIOS.swift:103`, `ExactViewIOS.swift:77`.

Both registrations list `UITraitPreferredContentSizeCategory` and `UITraitDisplayScale` only. `UITraitLegibilityWeight` changes a `UIButton` and `UIDatePicker` intrinsic size and emits no batch. The next empty poll skips `controls.sync`, so the kernel keeps the old size until some later batch has ops.

**Fix:** Add `UITraitLegibilityWeight.self` to both registrations.

Suppression paths in the projection, for a batch that is applied:

- **Create and update** of an option, a face child, or a run under one set `batch.controls` and return (`paragraph.rs:210`, `paragraph.rs:277`). `option_part` (`paragraph.rs:11-30`) walks text ancestors to an `option`, and treats any node whose parent is a native button as part of the face, which includes a direct `image "symbol:…"` child. The non-text bail is after that parent check.
- **Children.** `emit_children` on a `Control` sets the flag and returns (`paragraph.rs:332-335`). A `when` that swaps a face is a `SetChildren` on the button, so the button is touched and this runs. The new child also hits create.
- **Destroy.** The destroy arm does not set the flag (`host.rs:1169-1180`). It does not need to for an applied batch: destroying a child touches its parent (`txn.rs:568-574`), and that parent is either the control (`emit_children` sets the flag) or a text node still inside the button or option (`update` sets it). A face child that was viewless from birth also emits a `destroy` op, so `ops` is non-empty either way.
- **Kernel writes that never call `update`** (`tick`, `set_intrinsic`, `text_ready`) do not change a face or a menu. Writes that do (inherited `text-transform`, locale, a `rem` change) put the node in `touched`, and `update` sets the flag.

**Before landing:** finding 1. The session has to deliver the batch the flag was added for.

**Deferred:** findings 2 and 3 (same stale-size class as round 2, narrower). Also deferred: the spurious `destroy` op for a viewless id, which is pre-existing and harmless, and splitting `controls.rs` so a symbol change, a `when` swap, and an option removal each assert the flag on their own. Those three already set it.

Verdict: DO NOT LAND
