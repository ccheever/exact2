# Code review: Swift and Kotlin bindings, and the sample host (Tamber stage 5, commit 5), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `2fc030f3a199b1c75feb732246b8f9216817679288fec2c1d2901dbf9ad5698b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (Android scroll lends; the first batch after an unmounted observation; reparenting redraw; iOS wide clamp; a binding made in the factory).
- **Disposition:** applied without re-review (the last round).

---

I'll review round 3 read-only: the stage 5 brief, the round 2 commit, and the Swift and Kotlin binding fixes, without touching the worktree.The Android lend fix is incomplete, so a scrolled row can keep the previous element’s colour. The other round 2 items (redraw on an inputs change, no-stream `turned()`, cached tint, locks, last-asked place on exact2’s thread) hold on the paths they cover.

**[P1] Ask again after a scroll lends a view — `host/android/canvas/src/main/java/dev/exact/canvas/ExactView.kt:464`**

`Tambers.placed()` runs only when the drained publication included a paint. A move-only publication still calls `placeFeed()` (line 462), and `scrollBy` calls it directly (line 683). Both reach `Natives.near()`, which parks a pooled module view and lends it to another element. `check()` keeps comparing the previous element, and nothing invalidates the view, so `observe` does not run and the recorded node keeps the old colour. A fling is mostly these move frames: a row under a different pin draws the colour of the row that had the view before.

**[P2] The first batch after an unmounted observation is swallowed — `host/apple/Sources/ExactKit/Tamber.swift:262`**

`observe` while the view has no node stores `seen == nil`. The next `moved()` writes the new inputs and calls the body only when `seen` is already set. Observe, add the view to the window, then `setMode` once: the callback does not run, and the colour stays at the fallback. The new Mac test asserts that miss (`heard == 0` on the first `setMode`). A second change does notify.

**[P2] Reparenting does not redraw a held colour — `host/apple/Sources/ExactKit/Tamber.swift:253`**

Redraw runs only when the node id now under the view is in the set of nodes whose inputs changed. Move the view from a warm node onto a cool one during a batch that does not change the cool node’s inputs: `observe` runs (`seen` differs), but `needsDisplay` is not set, so a dynamic colour already on that view keeps painting the warm value.

**[P2] An iOS module colour clamps wide components — `host/apple/modules/ExactTamber.swift:56`**

The hatch sends extended sRGB components that sit outside 0–1 for a wide colour. `UIColor(red:green:blue:alpha:)` clamps them, so a Display P3 token in a module is ordinary sRGB. The Mac branch uses `extendedSRGB` and keeps the gamut.

**[P2] A binding made inside `factory.make` never attaches — `host/apple/modules/ExactTamber.swift:84`**

`create` registers the view with `Tambers.made` only after `factory.make` returns (`ExactNativeModule.swift:1022`). `ExactTamber(self)` during `init` captures `place == nil`, and `observe` then drops the callback. Later batches do not redraw that colour or call that observation. The sample avoids this by observing from `viewDidMoveToWindow`.
