# Code review: iOS host batch cost (f3de06ddd), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `f3de06ddd`.
- **Method:** one brief (sha256 `00dd62cd7145095bc0dd4f78b6c086386618199cf4bf693dfa03f7564e1dca26`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r2):** 1 (faces from viewless children) is taken by skipping only empty batches. 2 (appearance) is taken with stale marking on trait changes. 3 (title equality and Dynamic Type) is taken by dropping the early return. 4 (cache scale) is taken: the key carries the renderer scale.

- **Superseded (2026-10-04):** the branch these reviews were of was abandoned after round 3 and not landed. See `code-2026-10-04-batch-cost-r3.*` and `code-2026-10-04-idle-tick.*`.

---

1. **Blocker** — A button’s title or symbol, and a select’s menu, stay stale after the batch that changes them. `ControlsIOS.swift:128`, `PresenterIOS.swift:714`, `PresenterIOS.swift:883`.

`sync(changed:)` skips a configured control whose id is absent from `touchedAndAbove(touchedIDs)`. That set is the ids of `create`, `props`, `style`, `children`, `paragraph`, `flow`, and `frame` ops, plus their UIView ancestors. A native button’s title and symbol, and a select’s options, are viewless kernel children (`paragraph.rs:7-30`). `update` returns immediately for those nodes (`paragraph.rs:273`), and `emit_children` returns immediately for a `Control` (`paragraph.rs:327-329`), so a text or symbol change, a `when` that swaps the face, or an added option produces no op on the control. The kernel frame also stays put until the host reports a new intrinsic size, and that report only happens inside the configure path this skip bypasses, so a shrink-wrapped button never gets a follow-up `frame` op either. The UIButton keeps the old title and the pop-up keeps the old menu until some later, unrelated op hits that node. `testABatchConfiguresOnlyTheControlsItTouched` (`NativeButtonsIOSTests.swift:68-82`) never catches this: it changes the face closure and sends a `props` op on the button, which is exactly the case the skip still configures.

Fix: in the Apple host, when a touched node is a button, a select, or a viewless descendant of one, emit an op the presenter already counts (an empty `props` op is enough) on that control id if this batch did not already emit one. Keep skipping controls whose configuration is only their own props, style, and frame (checkbox, switch, slider, date). Extend the test with a button whose text child changes and a select whose option label changes, with no props, style, or frame op on the control, and assert the `UIButton` title and the menu update.

2. **Should-fix** — A `light-dark()` accent stays the colour of the previous appearance. `ControlsIOS.swift:140`, `ControlsIOS.swift:128`, `NodeViewIOS.swift:495`.

`accent_color` is stored as a pair and resolved with `channels`, which follows `drawsDark`, into a plain `UIColor` (`TextEngine.color`). The style JSON does not change when the appearance changes, so the scheme batch does not touch the control, and the skip leaves `tintColor`, `onTintColor`, and the checkbox fill as they were. The node’s own trait callback re-applies the node’s style and does not configure the control. An untouched switch, slider, checkbox, or button keeps the old tint for good.

Fix: remember the `userInterfaceStyle` last configured for each id, and do not skip when it differs. Clear that entry in the same places `synced` is cleared. The scheme batch then reconfigures only the controls whose trait actually changed, including a control inside an overridden sheet.

3. **Should-fix** — `HeaderTitleView.update` treats an incomplete equality as “nothing to draw”. `NavigationTitleIOS.swift:114`, `NavigationTitleIOS.swift:50`.

`HeaderTitle.source` is `id`, avatar source, subtitle, and tap. `update` also writes `accessibilityIdentifier` from `testId`, which is not in that string, so a testId-only change returns at line 114 and VoiceOver and the agent keep the old identifier. The same return skips `invalidateIntrinsicContentSize`. The labels follow Dynamic Type (`adjustsFontForContentSizeCategory`), but the title view’s intrinsic size does not, and an empty batch no longer invalidates it, so an accessibility size clips the subtitle.

Fix: add `testId` to `HeaderTitle.source`. Register the title view for `UITraitPreferredContentSizeCategory` and call `invalidateIntrinsicContentSize` there.

4. **Nit** — The avatar cache key omits the scale it was rasterized at. `NavigationBarIOS.swift:155`, `NavigationBarIOS.swift:160`.

`source` covers text, symbol, both colour pairs, and corners, and the point size is the constant 36. `UIGraphicsImageRenderer` bakes in the screen scale, and that scale is not part of the key. After a display-scale change the cached bitmap is reused indefinitely. `NSCache` is the right store: it is thread-safe and drops under memory pressure.

Fix: include the renderer scale in the key, and from the existing `UITraitDisplayScale` registration drop or bypass the cache so the title view assigns a newly drawn image.

What the skip does cover: an inherited `accent_color` or `text_color` is pushed by the kernel onto the control (`propagate_inherited`) and resent as a style op, so the control id is in `touchedIDs`. The control’s own `disabled`, `checked`, `value`, `min`, `max`, padding, and frame are props, style, or frame ops on that id. `inert` is read at activation, not baked into `isEnabled`. Glass reparents the button in `glassGroups.reconcile` after sync. Leaving controls are still removed by the live-id loop, which does not consult `changed`. A restart clears `controls`, and the next creates are in `touchedIDs`. Segments, tab bars, and menu rows read `face` from the kernel in their own full sync, so those projections stay current while the `UIButton` does not. The new tests prove the empty-batch skip and that an unchanged title view keeps the same avatar instance. They do not prove the superset claim in §9.

Verdict: DO NOT LAND
