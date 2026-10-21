# Code review: Swift and Kotlin bindings, and the sample host (Tamber stage 5, commit 5), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `2c5ed80bfa10134b92376d4bb901710130b75ef471f9e4e663f23a0e2b8ef9a7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (tint not in the inputs; off-main fallback; off-main tint; observe froze its place; Android no-stream turns; Kotlin's place walk off the main thread).
- **Disposition:** in the second-round commit.

---

I'll review round 2 of the Tamber bindings read-only: the brief, the round-1 fixes, and the diff from HEAD~1 to HEAD. No worktree changes.This is a read-only defect review of the round-2 delta. I'll read the brief, the round-1 notes, and the HEAD~1..HEAD diff without touching the worktree.The round-2 diff is seven files. Next I’ll check each claimed fix against the brief and the current binding code.The round-1 tint, stale-colour, and Android refresh fixes are still incomplete. Six defects are blocking.

**[P1] A module colour bakes the tint once, then keeps it while the accent changes — `host/apple/Sources/ExactKit/NativeHatches.swift:170`**

`hatchTamber` resolves every side with the node’s tint and the library stores that RGBA. `TamberPlace.inputs()` (`host/apple/modules/ExactTamber.swift:120`) replaces those sides only when the host `inputs` string changes. That string is still the runner fingerprint (tuple, design generation, scheme, contrast, root font size) and does not include the ancestor accent. Change `accent-color` from blue to red, or change the view’s `tintColor`, without `setMode`, a pin, a scheme change, or a font-size change: the next `color()` and `observe` callback both see the same `inputs` and paint the old blue.

**[P1] An off-main provider still gets the fallback or the old colour — `host/apple/modules/ExactTamber.swift:107`**

`sides()` talks to the host only on the main thread. `inputs()` on a move sets `known` to `(inputs, [:])`, dropping the RGBA. `moved()` does that before observers run. A renderer that then resolves the retained `NSColor`/`UIColor` off the main thread misses the empty cache and returns nil, so the dynamic colour becomes the fallback. A place with no observer is not touched by `moved()` at all; an off-main read sees `checked != batches` and returns the previous sides. `setMode` either clears the swatch or leaves it on the old colour until something on the main thread calls `sides()` again.

**[P1] In-process `@tint` is still dropped off the main thread — `host/apple/Sources/ExactKit/Tamber.swift:58`**

The provider passes `tint: nil` whenever it is not on the main thread. `SystemColor.channels` then uses the theme or system accent, not `nearestAccent` or the view’s `tintColor`. The same retained dynamic colour, resolved by AppKit off the main thread, paints that accent instead of the authored one. The main-thread prefetch only warms the unresolved `BatchValue`.

**[P1] `observe` still freezes the place — `host/apple/Sources/ExactKit/Tamber.swift:143`**

`color()` re-walks to the nearest node on each ask. `observe` records the id from `found()` once and, if the view is not in a node yet, returns without registering. Create the binding, call `observe`, then mount the view: `setMode` never calls the closure, while `color()` does follow the new place. Move the view under another pin after a successful `observe`: the closure stays subscribed to the old id.

**[P1] Android bindings are not checked on a turn with no new stream — `host/android/canvas/src/main/java/dev/exact/canvas/Owner.kt:403`**

`Tambers.turned()` used to run at the end of every `produce()`. It now runs only inside `if (fresh != null)`, before `queue.add`. `Native.frame()` returns null when the op stream is unchanged. A `setMode`, accent, scheme, or root font-size change that does not alter canvas ops never calls `check()`, so `observe` does not run and `color()` keeps the previous `Color` int. The `fresh == null` branch that still publishes scrollers, fields, bars, or accessibility (line 386) skips the check too.

**[P1] Kotlin re-resolves the place on exact2’s thread — `host/android/canvas/src/main/java/dev/exact/canvas/Tamber.kt:79`**

`check()` runs on exact2’s thread and now calls `place()`, which walks `View.parent` and `Natives.byView` (`Natives.kt:184`), a plain `HashMap` documented as main-thread. Main can be in `ensure` or `published` at the same time, so the walk can crash or return the wrong id and `take()` then stores that node’s colours. It is also too early to see a lend: reparenting happens on main in `published()`, after `turned()`. The turn that moves the view still checks the old node, so a new pin does not refresh or notify until a later turn.

These round-1 items hold. Wide colours cross as four extended-sRGB components (a Display P3 red and a linear value of 4 both convert out of range on this Mac, including headroom-tagged colours). Unresolvable sides are `[]`, so the library uses the fallback instead of transparent black. Module colours are built in extended sRGB. Every observer of one place is told, and `inputs()` runs once. Hatched element views are registered in `Tambers.made` before `element()`. A retired in-process place drops its colour cache when `tamber` returns nil. The Linux accent is part of the inputs string, so a stable place does see accent changes when `check()` actually runs.

Blocking until the six above are fixed. The Mac sample can still pass: it observes and re-applies on the main thread, and it does not change tint without also changing `inputs`.
