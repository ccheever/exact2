I'll review the tip commit against the branch diff, starting with AGENTS.md and the review instructions so the check stays read-only.The review stays read-only. Next I’ll read the binding rules and the tip commit against the branch diff.The global rule is in place and nothing blocks landing. Viewport height units and segments are the window’s for the whole session, and the paths that still resolve a length all go through that.

## Should-fix

**`QUEUE.md:113` — the remaining SVG/canvas fix points at a deleted API.** `kernel/src/svg/length.rs:108` still turns `height="50vh"` into 0, and `kernel/src/svg/scene.rs:248` (`content_box`, also `host/apple/src/canvas2d.rs:83`) still drops `padding-top="10vh"` from the drawing box. The queue says to resolve both through `NodeArena::env_for`. This commit deleted `env_for`, `Env::screened`, and `Screen`. A nested SVG with `height="50vh"` in an 844-point window still disappears, and an implementer who follows the queue does not compile. Resolve with `Dimension::resolve(arena.env())` (`ViewportUnit::basis` already reads `Env::screen`) and use the laid-out padding. Do not put `env_for` back.

**`ExactViewIOS.swift:317` and `Agent.swift:467` — the host’s window segments are not locked by a test.** `fit` now folds `self` / `screenFrame`, and `prefer segments` divides `session.screenSize`, falling back to the presenter viewport only when that is nil. `segments_are_the_windows_at_any_sheet_height` (`kernel/tests/it/fit_content_native.rs:171`) installs the rects itself, so it stays green if `fit` folds the sheet container again or `prefer` divides the sheet. On a fold or `prefer segments 1x2`, a 400-point sheet would then read the sheet’s grid while `vh` stayed the window’s. Add a host test: screen 390×800, viewport resized to 390×400, and both a medium route and a fit-content route see the window’s segment height.

## Nits

**`host/apple/tests/it/fit_content.rs:559` — the comment describes the code this commit removed.** It says the trial re-resolves the parent’s padding in the route’s environment, and that an ancestor’s viewport width still follows the sheet. `geometry.rs` copies the laid padding and border, which is correct because that layout already used `Env::screen`. An ancestor `width: 50vh` is the window’s too. Drop the parenthetical.

**`llp/1075.003-native-platform-control-merged.plan.md:1411` — the spec contradicts itself.** Lines 1386–1403 say the global rule replaces option (b). Lines 1408–1412 repeat the rule and then say the lead chose option (b), which was the per-route screen. Delete the repeated sentences.

**`docs/contract-for-humans.md:1216` — `vmin`/`vmax` are not “the window’s height”.** `viewport.rs:61-72` takes min and max of both window axes. On a 390×844 window, `10vmin` is 39 and `10vmax` is 84.4, which `the_screen_is_what_the_height_units_read` already locks. Say smaller and larger side.

## Checked

Layout, the trial, native field floors, button measurement, and the painted face (`button_face_style` → `exact_press_face`) all use `arena.env()` and `ViewportUnit::basis`. So do multicol `column-width`, the region certificate (`unpadded`), sticky insets, and painted style JSON. `vw` stays the layout viewport (`10vw` is 30 against a 300-wide offer while the window is 390). With `screen` set, `vmin`/`vmax` use the window’s width and height, so on a sheet narrower than the window `1vmin` can exceed `1vw`. That matches the rule and the test.

A medium, large, or point sheet’s `vh` is the window, not the sheet. In an 800-point window with the viewport at 400, `the_window_is_every_sheets` expects both the medium route and the fit-content route at 400. No app, fixture, or conformance contract in the repo puts a viewport-height unit on a sheet route. The web conformance `5vh` / `1dvh` rows are on the root. `apps/photo-editor`’s `100vmax` is browser CSS.

Rotation, split view, and window resize go through `fit`: `exact_screen` is sent before `resize` when the window size changes, and `set_screen` restyles every `uses_env` node (viewport units, comparisons, segments). `rebooted` still sends `lastScreen` before insets, segments, and `fit`. The layout inside `runtime.boot` is still one pass with no screen; the resend in that same turn corrects it. That order is unchanged.

Web, Linux, and macOS never call `exact_screen`. With `screen == None`, `basis` is the layout viewport, as before. tvOS shares this view, but with no sheet and no keyboard shrink the window size and the layout viewport are the same.

`env_for`, `rescope`, the screen’s own segment grid, and the stand-in’s re-resolved padding and content-box width adjustment are not needed. Laid edges already used the same environment. The stand-in parent and the flex-row siblings stay, and `a_width_through_a_percentage_height_does_not_cycle` and `a_rows_siblings_set_the_routes_width_in_the_measure` still require them.

By inspection, these fail if `basis` reads the sheet again: `50vh` → 422 rather than 100, `10vmin` → 39 rather than 20, the field floor and the button face staying 844 / 84.4 at sheet heights 200 and 44, and the medium route at 400 rather than 200. Tests were not executed (`cargo test` would write `target/`).

Nothing blocks landing.
