# Code review: aria-modal (f8c924003), 2026-10-03 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `f8c924003`.
- **Method:** one brief (sha256 `c8c7f32b67a8f605b0dc6450fb745e6fd744dd00891b32733b38502eb171e017`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's findings and verdict, unedited. The progress lines before them are omitted.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):**
  - 1 (hidden or departing dialog stays modal; no move back): taken in `syncModal`.
  - 2 (macOS modal inside wrappers): taken by deferring macOS (D3).
  - 3 (the bars are siblings only when the root's container is the root view): taken. LLP 1080.003 D2 states the condition. The clone's root clips with `clipsToBounds` and has no clip box, so its overlays qualify.
  - 4 (two modal siblings): taken. Only the frontmost modal sibling, by `zPosition` and then subview order, carries the property.
  - 5 (tests claim more than they check): taken for iOS. The test covers clear, `display: none`, sibling modals and exit. Pool reuse is the existing reset line.

---

1. **Should-fix** — A hidden or departing dialog stays modal, and closing it does not move VoiceOver back. `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1020`

`accessibilityViewIsModal` follows `props["accessibilityModal"] == "true"` and nothing else. `display: none` publishes a zero frame and leaves the view in the hierarchy with `isHidden == false` (`kernel/src/layout/publication.rs:78`), so a dialog written `aria-modal=true` and shown with `display` keeps hiding its siblings after it closes. The same-value guard at `1021` does not repost, and it does not clear the flag. A presence exit sets `accessibilityElementsHidden` and leaves the modal flag set (`host/apple/Sources/ExactKit/IOS/PresenceIOS.swift:28`), so during the exit both the overlay and its siblings are skipped. Destroy removes the view (`host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:796`) without posting `.screenChanged`. The notification runs only when the bool flips on a view that stays mounted, which is the open of a conditional overlay and not its close. Repeated `applyProps` with the same value is fine. macOS has the same `display: none` hole: `NodeViewMac.swift:347` checks `isHidden` only.

Fix: treat the view as modal only when the prop is true, `style["display"] != "none"` on it and its ancestors, and it is not `isHidden`. Post `.screenChanged` when that effective value changes. In `beginExit` and on remove-from-window, clear the flag and post `.screenChanged` with `nil`. On macOS, ignore a `display: none` candidate in the `last(where:)`.

2. **Should-fix** — On macOS, a modal inside a scroll, clip, material, or glass parent exposes nothing. `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:343`

The modal is sought in `container.subviews`, but the list that gets filtered is `super.accessibilityChildren()`, which is `self`'s subviews. Those differ whenever `container` is not `self` (`GlassGroup.swift:343`: document view, canvas overlay, material content, or clip box). A scrolling parent moves its children into the document view (`NodeViewMac.swift:1040`) and its accessibility children are the scroll view. The scroll view is not inside the modal, so the filter returns `[]`. `NSAccessibility.unignoredChildren` then has nothing to flatten, and both VoiceOver and `tree --ax` (`AgentAccessibility.swift:290` calls this override) drop the modal and its siblings. For a plain parent the filter is sound: a button's `nil` children only make that button a leaf, inline runs follow `accessibilityParent()`, and a nested modal is filtered again on the inner view.

Fix: if `container === self`, keep the current filter. Otherwise return the chosen modal view itself (not the scroll, clip, or glass wrapper) so unignored-children expansion exposes only that subtree.

3. **Should-fix** — The tab and navigation bars are siblings of the overlay only when the root's `container` is the root view. `host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:140`

`mount` adds the tab controller with `root.addSubview(holder.view)`. `placeChildren` inserts the overlay into `parent.container` (`PresenterIOS.swift:883`). `container` is the root view only when the root has no scroll, clip box, material content, or glass group (`NodeViewIOS.swift:526`, `GlassGroup.swift:212`). Otherwise the overlay sits inside that wrapper and the bars are siblings of the wrapper, which `accessibilityViewIsModal` does not hide. A non-scrolling root matches the simulator result. A route-local dialog also does not hide the bars: the route view lives in `RouteController.view`, and the bars are not its siblings. Flat leaves cannot hold the prop (`FlatLeavesIOS.swift:73` promotes anything but `dataset`). Pool reuse does clear the flag (`NodePoolIOS.swift:512`).

Fix: say this in the LLP, and only claim the bars when the overlay's superview is the chrome view's superview. Setting the flag on the scroll view instead would hide the bars and keep the scrolled page, because that page would be inside the modal view.

4. **Should-fix** — Two sibling modals: `tree --ax` keeps the back one, and UIKit's rule hides each of them. `host/apple/Sources/ExactKit/AgentAccessibility.swift:214`

The walk uses `kids.first` in subview order, which is the rear modal, then skips every other sibling, including a menu drawn later. macOS keeps the last subview (`NodeViewMac.swift:347`). With both flags set, each view hides its siblings, so the two overlays hide each other.

Fix: on iOS set `accessibilityViewIsModal` only on the frontmost visible modal sibling and clear the others. In the agent walk, use that same frontmost owned modal instead of `kids.first`.

5. **Nit** — The tests and the LLP claim more than they check. `host/apple/tests/ExactKitTests/AccessibilityTreeIOSTests.swift:123`

The iOS test never reuses a view, and after `clear` it checks the flag but not that `"named"` is exposed again. The LLP says both (`llp/1080.003-authored-modality.rfc.md:37`). The macOS test is a plain parent only. "The last one, as it paints on top" (`NodeViewMac.swift:346`) is the last subview, not the highest `zPosition` (`NodeViewMac.swift:1103`). Web `aria-modal="false"` is emitted as the ARIA token and is harmless (`element.rs:802`, `document.rs:578`); the JS target gets that same map (`web-js/src/rows.rs:512`, `rt.js:578`) and `agent.js:43` reads `"true"` the way it reads `aria-hidden`. The compiler accepts a bool on any element, same as `inert`, and `_comment` is stripped before the digest (`kernel/build.rs:1454`).

Fix: assert exposure after clear, cover pool reuse, a `display: none` modal, and a modal inside a scrolling parent. Pick the macOS modal with the highest `zPosition`, then the later subview.

Verdict: LAND WITH FIXES
