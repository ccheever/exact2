# Code review: iOS host batch cost (f3de06ddd), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `f3de06ddd`.
- **Method:** one brief (sha256 `00dd62cd7145095bc0dd4f78b6c086386618199cf4bf693dfa03f7564e1dca26`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r2):**
  - 1 (viewless children change faces with no op on the control): taken. r2 drops per-control skipping. It skips the control sync only for a batch with no ops, and any batch with ops configures every control as before.
  - 2 (appearance): taken. A trait change marks the controls stale, and so does a text-size or display-scale change; the next batch then syncs them, empty or not.
  - 3 (deferred geometry replay): taken. `applyGeometry` marks the controls stale.
  - 4 (title equality omits testId): taken by dropping the title view's early return. The avatar's image cache keeps the redraw cheap.
  - 5 (cache budget): taken, a count limit of 64. 6 (`synced` on restart): gone with the per-control set.

- **Superseded (2026-10-04):** the branch these reviews were of was abandoned after round 3 and not landed. See `code-2026-10-04-batch-cost-r3.*` and `code-2026-10-04-idle-tick.*`.

---

Static review only; no files changed or tests run. **The touched-view set is not a sufficient invalidation set.**

1. **Blocker — Changes to viewless children leave buttons and select menus stale.**  
   [ControlsIOS.swift:128](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:128) skips controls whose IDs are absent from `changed`. However, the Rust projection suppresses native-button children and select options: [paragraph.rs:273](/tmp/x11-review/host/apple/src/paragraph.rs:273) returns without emitting their updates, and [paragraph.rs:327](/tmp/x11-review/host/apple/src/paragraph.rs:327) suppresses control child lists.

   Changing only a button’s child text or symbol, or an option’s label/value/disabled state, therefore need not produce any operation on a projected view. An unchanged control frame will not rescue it. The old face/menu—and potentially its accessibility label and intrinsic size—remains.

   **Fix:** Emit an owning-control invalidation or revision whenever its viewless contents change, including structural changes. Consume that in `changed`. Retain full synchronization until that path exists. Test actual Contract→Rust batches with fixed control geometry.

2. **Should-fix — Appearance changes no longer reliably refresh resolved accents.**  
   [ControlsIOS.swift:140](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:140) resolves `accent_color` into a fixed `UIColor`. The trait callback at [NodeViewIOS.swift:495](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:495) reapplies the node’s style but does not synchronize its control. Scheme reporting handles paint motion; it does not guarantee a control operation for an unchanged `light-dark()` accent.

   Consequently, switching appearance can leave buttons, switches, checkboxes, sliders and date/select controls using the previous accent, including after unrelated batches.

   **Fix:** Explicitly invalidate affected controls on local appearance changes, then synchronize after traits settle. Also provide explicit intrinsic-size remeasurement on Dynamic Type changes instead of relying on incidental style/frame operations. Cover system changes, local overrides and agent-driven appearance changes.

3. **Should-fix — Deferred geometry replay is outside the new invalidation set.**  
   [PresenterIOS.swift:883](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:883) derives `changed` only from the current batch. During navigation synchronization, modal dismissal can replay older frame operations through [ModalIOS.swift:243](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:243).

   For example, change a background button’s fixed width while a sheet is open. Its frame is deferred, so control synchronization uses the old bounds. Dismissal applies the saved frame, but the dismissal batch need not touch that button. `applyGeometry` does not resize its `UIControl`, leaving the control at its old size or position.

   **Fix:** Record control geometry invalidations when frames are actually applied, including replay, and merge them into the post-navigation synchronization. Test resizing a background button or slider before dismissing a modal.

4. **Should-fix — `HeaderTitle` equality omits a property that `update` writes.**  
   The early return at [NavigationTitleIOS.swift:114](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:114) relies on equality defined through `source`, which omits `testId` at [NavigationTitleIOS.swift:50](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:50). Changing or clearing only the group’s `testId` leaves `accessibilityIdentifier` stale.

   **Fix:** Use synthesized fieldwise equality, or explicitly include `testId`; keep corresponding source signatures consistent. Test both replacement and removal with unchanged title/avatar/subtitle.

5. **Nit — The process-wide avatar cache has no configured memory budget.**  
   [NavigationBarIOS.swift:159](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:159) retains images for distinct text/symbol/colour/corner combinations. Dynamic colours or many visited avatars can create many entries. `NSCache` may evict them, but the implementation supplies neither a count limit nor a cost limit.

   **Fix:** Set a modest count or total-cost limit; account for both raster variants when assigning costs.

6. **Nit — Restart leaves `synced` bookkeeping behind.**  
   [ControlsIOS.swift:254](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:254) clears the other control maps but not the newly added set. This does **not** currently skip recreated controls—the `controls[id] != nil` condition prevents that—but retains obsolete IDs across restarts.

   **Fix:** Add `synced.removeAll()` to `reset()`.

The remaining paths have useful safeguards:

- **Inherited styles:** The kernel explicitly touches inheriting descendants ([txn.rs:1131](/tmp/x11-review/kernel/src/txn.rs:1131)), and Apple serializes inherited rows for controls. Ordinary ancestor `accent_color`/`text_color` changes therefore reach the control.
- **State and geometry:** Own disabled/value/padding/style/frame operations are included. Inertness uses ancestor interaction blocking and live activation checks. Slider/date/select callbacks also explicitly reconcile committed values after interaction.
- **Lifetime and mounting:** Controls are neither flat leaves nor reused pooled controls; new IDs receive create operations. Exit retention and cleanup remain unconditional. Glass-slot creation/removal moves the control itself. Ordinary retained-route remounting preserves configuration; trait changes and deferred geometry remain the exceptions above.
- **Ordering:** Keeping navigation and segments before controls is sensible. Navigation reads native faces from the kernel. The concrete ordering problem is geometry replay during navigation, not merely computing `changed` earlier.
- **Header/cache completeness:** Avatar colours, corners, text, symbol and tap identity are represented; image size is fixed at 36 points. Both appearance variants are registered, so an unchanged avatar should not require rerasterization merely for light/dark mode. Labels already opt into Dynamic Type; wrapper intrinsic-size invalidation still deserves an explicit trait test. I found no concurrent rendering caller; `NSCache` access itself is thread-safe.

The new tests prove much less than §9 claims. [NativeButtonsIOSTests.swift:68](/tmp/x11-review/host/apple/Tests/ExactKitTests/NativeButtonsIOSTests.swift:68) substitutes face data and manually touches the control, bypassing the missing projection invalidation. [NavigationBasicsIOSTests.swift:105](/tmp/x11-review/host/apple/Tests/ExactKitTests/NavigationBasicsIOSTests.swift:105) proves image identity survives an empty batch, but cannot distinguish the title’s early return from an image-cache hit. Neither covers the correctness failures above.

Verdict: DO NOT LAND
