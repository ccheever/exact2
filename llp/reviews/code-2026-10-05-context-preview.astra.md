# Code review: context menus with a preview and a commit (LLP 1021 §5.1), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /private/tmp/x2-peek-review`.
- **Method:** one brief (sha256 `8c822ce06355327fa9ad6e2c71113dd7be34eafb6e88b22e3a65ee3e010eda9f`), shared with grok. Round 1, blind to the other review. Reviewed the uncommitted diff in a detached worktree at `a51d6ecab`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken: a source that is unmounted or stops naming the popover now dismisses the menu and returns the row at once, and `willEndFor` matches its own configuration object rather than the source's id (`ContextMenuLifetimeIOSTests`). 2 taken: the row returns into its popover's container as it is then. 3 taken: the commit checks the popover's authored ancestry (disabled, inert, inactive route) and the source's eligibility (test: an inert popover refuses). 4 taken on both web targets: the popover is resolved after the action, in the timeout. 5 taken: any frame that differs from the one last placed is the new home (test: an origin-zero resize). 6 taken: the listener checks the attribute before it prevents anything. 7 taken: only a commit that navigated drops the dismissal target. The noted coverage gaps (UIKit's own closures, the macOS right-mouse path) are in §5.1's Evidence and QUEUE.

---

1. **[P2] Removing an interaction can strand its preview.** [ContextMenusIOS.swift:66](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:66) removes the interaction before line 85 searches for it to call `dismissMenu()`. When the source is destroyed or loses `contextPopover`, that lookup cannot succeed. There is no restoration fallback if `willEnd` does not arrive; moreover, `willEnd` rejects cleanup if the weak source has disappeared. The lifted row can remain outside its popover until another successful configuration or reset.

2. **[P2] Restoration uses an obsolete UIKit container.** [ContextMenusIOS.swift:175](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:175) restores into the saved `UIView`, rather than the popover’s current container. Changing the popover’s glass/material while the preview is lifted can replace that container. The material code moves only children physically present, so it misses the lifted row. Dismissal then inserts the row into the old container—or detaches it permanently if that weak container disappeared. Subsequent menus cannot find it.

3. **[P2] Commit eligibility loses the authored ancestors.** [ContextMenusIOS.swift:205](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:205) calls `menus.eligible(row)` after moving the row into a standalone preview controller. That check follows UIKit ancestry, so it no longer sees the popover’s inert ancestors or original route. Making the popover inert while the menu is open therefore still permits its preview action. `current(owner)` also does not recheck source eligibility. Eligibility needs to include the saved logical ancestry.

4. **[P2] Both web targets resolve the popover before the action runs.** [rt.js:727](/private/tmp/x2-peek-review/host/web-js/rt.js:727) and [glue.js:435](/private/tmp/x2-peek-review/host/web/glue.js:435) capture `p` immediately; only `showPopover` is deferred. These listeners are installed before the authored handler. If that handler changes the target, the old popover opens; if it creates or replaces the popover, the new one never opens. This contradicts §5.1’s action-before-read ordering. Both failures reproduced using the actual listener bodies.

5. **[P2] Frame updates at the origin are discarded when saving restoration geometry.** [ContextMenusIOS.swift:94](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:94) treats a nonzero origin as evidence of a frame op. A legitimate update from `(0,0,300,180)` to `(0,0,300,240)` leaves `home` unchanged. Dismissal restores the old height, despite the kernel already having delivered the new geometry. Removing popover padding can similarly restore an obsolete offset.

6. **[P2] Clearing a web binding leaves an event-intercepting listener.** The same [rt.js:727](/private/tmp/x2-peek-review/host/web-js/rt.js:727) and [glue.js:435](/private/tmp/x2-peek-review/host/web/glue.js:435) handlers set `$cp` and prevent the default before checking whether `contextpopover` remains meaningful. An empty or cleared child binding therefore blocks an ancestor’s valid context menu and suppresses the browser menu. This also reproduced for both targets.

7. **[P3] Non-navigating commits incorrectly lose their dismissal target.** [ContextMenusIOS.swift:208](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:208) sets `committed` before knowing whether navigation changed, while line 198 suppresses the target for every committed preview. A `.dismiss` commit consequently loses its return target too, contrary to §5.1’s navigation-specific rule.

The tests cover basic extraction and manually invoked delegate paths. They do not cover frame/children updates, container changes, invalidation, recycling, missing callbacks, or delayed callbacks across presentations. The fake commit animator counts a closure; it does not prove UIKit’s morph or an unanimated navigation push. The macOS test never exercises `MenuHost.context` or right-mouse delivery.

`git diff --check` passed. Native builds/tests were unavailable in this read-only checkout; no files were edited.

**Verdict: NOT READY.**
