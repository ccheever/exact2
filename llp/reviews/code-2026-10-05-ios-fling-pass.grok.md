# Code review: iOS, a fling no longer pays the presenter's whole pass per batch (5b33306bc..eee07e463), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `eee07e463`, plain output.
- **Method:** one brief (sha256 `ae2143c4e5406da1dc5df7367b5905a89422374df265ef75f713329a52e9bc22`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition (r2):** 1 taken: an empty batch with `timers` false goes through `applyUnlessEmpty` and does not apply. 2 taken (see astra's 5): the natural size for a longer title and a larger text size, the gradient's re-aim with the appearance flipped; a subtitle-only title change was already pinned. 3 taken: LLP 1079's note names `applyUnlessEmpty` and the scroll event, and why other events still apply.

---

LAND

1. MINOR — `IdleTickTests` still only skips a timer batch, so the widening this commit adds is unpinned. `host/apple/tests/ExactKitTests/IdleTickTests.swift:15` builds `timers: true`, and `Session.swift:942` skips on `changesNothing` alone. Restoring `if batch.timers && changesNothing` keeps every assertion green. Fix: `apply` one empty batch with `timers: false` and assert `appliedBatches` stays put and `timerDue` moves, and keep the existing ops / `controls` / error / motion cases.

2. MINOR — The three new iOS caches are not pinned, except button-face invalidation. `NativeButtonsIOSTests.swift:172` does fail a props touch, a sibling frame, or `controls: true` that leaves the old title. Nothing fails if `HeaderTitleView.update` (`NavigationTitleIOS.swift:214`) compares only the title string, if `NativeButtonIOS.naturalSize` (`NativeButtonsIOS.swift:34`) keeps the first size across a new title or a content-size trait, or if `reaimFixedGradient` (`BoxLayerIOS.swift:184`) reuses stops after `drawsDark` flips. Fix: one subtitle-only title update, one longer button title plus a second read after `preferredContentSizeCategory` changes, and one re-aim of the same gradient with `dark` flipped that expects new `CGColor`s.

3. NIT — LLP 1079 still says only the timer path skips, through a function this commit deletes. `llp/1079-speed-hygiene-in-the-agents-hands.rfc.md:653` (`scheduleClock` → `applyTick`). `Session.swift:1176` now calls `apply`, and any `changesNothing` batch returns at `Session.swift:947`. Fix: a short amendment that `apply` skips every such batch and `applyTick` is gone.

The skip is the old `applyTick` guard on every batch: not while `applying`, not while a fill, tick, or canvas is in flight, and only when `changesNothing` (`Session.swift:925`) matches. Deadline changes still arm the clock. Face, title, and gradient caches key on the value that is drawn: `controls` or a touched id clears a face (`ControlsIOS.swift:128`), the content-size callback nils `shown` before rebuilding the subtitle (`NavigationTitleIOS.swift:166`), and a new layer, source, or `drawsDark` reparses stops (`BoxLayerIOS.swift:184`). `applyGradientLayer` drops the aim cache (`BoxLayerIOS.swift:155`). Scroll work still runs from `scrollViewDidScroll` before the handler's batch (`NodeViewIOS.swift:637`). Idle-tick r3 still holds: `requestProjectionSync` (`PresenterIOS.swift:35`) is grouped lists, then segments, then `controls.sync()` with `contents` defaulting true, and it does not call `prepare()`. Touched files are under the 1,500-line cap (`Session.swift` 1418, `PresenterIOS.swift` 1393).
