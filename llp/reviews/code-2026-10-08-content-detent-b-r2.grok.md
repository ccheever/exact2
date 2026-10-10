I'll review the last four commits read-only, starting with the project rules and the two reviews the tip commit is meant to answer.The review is read-only and scoped to the last four commits. I'll read both prior reviews and the tip diff before checking whether each finding is actually fixed.The tip claims to scope the screen per route, derive the trial width in a stand-in parent, and resend the screen after a reboot. I'll check those three against the current sources.The prior findings look fixed on inspection. I'll finish the unread resolve sites—native measure caches, paint paths, and the trial's imported parent padding—before ranking anything.The two Astra blockers and the Grok reboot finding are fixed. Nothing here blocks landing.

`env_for` (`kernel/src/arena.rs:168`) gives the window size only to a live `fit-content` route and its descendants. `ExactViewIOS.fit` stores that size from the window bounds before the resize, including when another detent is on top (`ExactViewIOS.swift:285-350`). The trial sizes the route in a stand-in parent and forces the route's height and its top and bottom insets to auto (`kernel/src/kernel/geometry.rs:136-173`). `rebooted` clears `lastScreen` before `fit()` (`ExactViewIOS.swift:388-394`). The docs now name `svh`/`lvh`/`dvh`. `emit_fitted` still resends on height only, still skips a fixed-length scroller that did not move, and still keeps presented heights. Web, Linux, and macOS are outside the diff and never set a screen.

The new tests lock both stack orders, a detent flip, a route whose first extent is measured while the viewport is already the sheet, and the `100↔300` percentage-height route. The r5/r6 cases (`min`/`clamp` collapsing to 0, `100vmax` on a child, negative `vh` margin, field `min-height`, button padding, width through a height unit) are fixed points against the screen. I did not run them. On the previous code those assertions fail: `vh` followed the resized viewport, a medium route saw the screen, and the trial copied the route's published width.

Percentages of the route and `flex-grow` take nothing in the trial, because the route's block size is indefinite there. A safe-area inset under `viewport-fit: cover` is still the sheet's, and `screened` does not replace it. `emit_fitted` subtracts the bottom cover. Crossing the home indicator is one step, not a cycle I can show.

## Should-fix

**The trial still imports a sheet-dependent containing-block width.** `kernel/src/kernel/geometry.rs:160-173` copies the parent's last resolved padding and border as definite lengths. Those were resolved against the sheet, because the parent is outside the route. A parent with no engine node, or a route that is the root, still uses the route's own published width (`geometry.rs:149`).

A page column `width: 100%`, `padding-left: 10vh`, with a stretched `fit-content` route (`left: 0`, `right: 0`) and wrapping text: `10vh` is 10% of the sheet height, the route's wrap width follows it, the content height changes, the sheet resizes. The percentage-height test's parent has no such padding, so it stays green.

Re-resolve the stand-in's horizontal padding and border from the parent's style with the block size indefinite (the screen, when the route is screened). Do not use the route's own `frame.width` as the containing-block width.

**Viewport-segment heights are still the top sheet's, for every route.** `ModalIOS.swift:246` (`fitsContent`) is `layers.last` only. `ExactViewIOS.swift:318-320` sends window segments only while that top sheet is `fit-content`, otherwise the sheet frame. `Env::screened` (`kernel/src/style/env.rs:301`) replaces the viewport size and leaves `segments` alone. The plan (`llp/1075.003-native-platform-control-merged.plan.md:1397`) calls this global on purpose.

On a fold, a covered `fit-content` route's `env(viewport-segment-height)` follows the medium sheet above it, while its `vh` correctly stays on the screen. A medium sheet under a `fit-content` sheet gets the window's segments. A phone has no segments, and the visible `fit-content` sheet itself does not oscillate.

Keep a screen grid and a viewport grid, and pick in `env_for` the same way as the viewport size.

**SVG and canvas viewport lengths are still 0.** `kernel/src/svg/length.rs:108` resolves only points, percents, and `calc`; anything else, including `vh`, is 0. `kernel/src/svg/scene.rs:248` (`content_box`, also the canvas clip at `host/apple/src/canvas2d.rs:83`) does the same for padding. A nested `svg` with `height: 50vh` disappears. `padding-top: 10vh` is in the layout and missing from the drawing box. Constant, so no detent loop. Already queued (`QUEUE.md:101`). Resolve both through `env_for` and the resolved padding.

**The painted native button face does not use the screen.** Layout measurement does (`kernel/src/layout/buttons.rs:134` and `:184`, and the host measures that request). `NodeArena::button_face_style` (`kernel/src/arena/button.rs:141`) calls `resolve_geometry(self.env(), …)`, and `exact_press_face` is what configures the `UIButton`. Padding of `10vh` on an 844-point screen is reserved as 84 points and drawn as 10% of the sheet. The box does not hunt, because the measurer does not read the live control.

Pass `env_for(slot)` in `button_face_style`.

## Nit

**Stale comments.** `host/apple/src/fold.rs:25` and `Session.swift:1298` still say the screen is installed only while a `fit-content` sheet owns the viewport and cleared otherwise. The host always sends a positive window size.

**One sheet-relative layout before the screen comes back.** `finishBoot` / `presentCommitted` apply the new kernel, then `rebooted`. `rebooted` sends insets and segments before `fit()` (`ExactViewIOS.swift:391-394`). `reset` moves the viewport home without resizing it, so that first layout can publish a content height against the old sheet frame with `screen` still `None`. The following `fit` sends the screen and corrects it. No test drives `rebooted`.

**Content-region certificate.** `kernel/src/region/state.rs:839` calls `unpadded(arena.env())`. `padding-top: max(0px, calc(50vh - 400px))` is 0 against a 400-point sheet and about 22 points against an 844-point screen, so a region inside a `fit-content` route can be admitted and then padded. Use `env_for`.

Nothing blocks landing.
