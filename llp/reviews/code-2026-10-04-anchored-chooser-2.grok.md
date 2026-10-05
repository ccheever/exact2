# Code review: the anchored chooser, part 2 — macOS, position-area, hr, compile-time refusal, 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree.
- **Method:** two rounds on the briefs shared with astra (round 1 sha256 `114640ba005da39373d4a67806cb44ad58604206d5a3bf22afc6256437e3f576`; round 2 sha256 `a0ed44ef0a92977d86da6ad116906e3cddaa2fae3155371924ee4a919d4b9820`), blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdicts, unedited.
- **Verdicts:** round 1 DO NOT LAND; round 2 LAND WITH FIXES.
- **Disposition, round 1:**
  1. *The web needs `inset: auto; margin: 0` with `position-area`.* Argued with measurements, in LLP 1021 "Placement". In Chrome 154, `position-area` resolves the popover's auto margins to 0. The fixture's popover is centred flush above its invoker, and so is an isolated page, even with `margin: auto; inset: 0` authored. Round 2's reviews agreed.
  2. *A press inside `NSMenu.popUp`.* Fixed. A choice is recorded and pressed on the next turn after revalidation. Plain button menus were fixed the same way.
  3. *The `NSMenu` ignores `position-area`.* Fixed: `popUpPoint` comes from `PositionArea.origin`.
  4. *A `link` row is accepted.* Fixed: it is refused.
  5. *`center` on the iOS sheet is undefined.* Defined (the whole invoker, no arrow) and documented, with UIKit choosing the vertical position.
  6. *Weak tests.* Fixed: real `popUp` tests, and agent-mode skips.
- **Disposition, round 2:**
  1. *Margins never reach Apple.* Fixed, as astra's round-2 #2.
  2. *Inset shades the resolved `color`.* Argued with measurements. In Chrome 154, a `currentcolor` side paints #9a9a9a over #eeeeee whatever `color` is (red, gray, black, #000040, and an explicit `border-color: currentcolor`), though computed style reports the resolved colour. An explicit colour is shaded from itself. The measurements are now tests, and are recorded in LLP 1001 "Border semantics".
  3. *A hidden button-menu row is enabled.* Fixed: the item is enabled only when the row is not disabled, not inert and shown.

## Round 1

**Blocker** — `host/web/src/css.rs:524` (both web targets; the JS target emits through the same `css_text`). A popover with `position-area` is pinned to the start of that area, so the foot-of-card chooser opens at the top-left of the viewport instead of above its button.

   The HTML UA stylesheet for `[popover]` sets `inset: 0` and `margin: auto`. `position-area` only resolves **auto** insets and **auto** margins to 0 and then uses those insets as offsets from the area (CSS Anchor Positioning §3.1). These hosts emit the `position-area` declaration and nothing else. The fixture popovers (`apps/native-fixture/app.contract:377` and `:381`) set `position="absolute"` and `position-area="top span-all"` and leave inset and margin alone, so in Chrome the specified `inset: 0` stays 0, auto margins are zeroed, and a width of 220 lands on the left edge of the top band. Painted layers do the other thing: `PositionArea.origin` (`host/apple/Sources/ExactKit/PositionArea.swift:24`) puts that same value above the invoker, centered. `llp/1021-menus.rfc.md:466` says Chrome places it there. The spec's own `select::picker` rule sets `inset: auto` next to `position-area` because the property alone does not.

   **Fix:** when a popover's `position-area` is not `none`, also emit `inset: auto; margin: 0` on both web targets, and assert the emitted CSS (or a browser box) sits above the invoker.

2. **Blocker** — `host/apple/Sources/ExactKit/Mac/ChooserMac.swift:138` (`choose`), with `PresenterMac.swift:738` and `Session.swift:670`. Choosing a macOS confirmation runs the app batch inside `NSMenu.popUp`, so Discard Changes destroys the view AppKit is still tracking.

   `popUp` does not return until tracking ends (`MenusMac.swift:131`). `choose` sets `finished` and calls `presenter.press` on that stack. `press` runs `onPress` before the captured hide command, and `onPress` is `apply(runtime.press(...))`, synchronous. Messages' Discard Changes is `press=discardContact` (`apps/messages-legacy/new-contact-sheet.contract:354`), and `discardContact` is `nav = back(nav)` (`app.contract:333`). That batch unmounts the new-contact sheet, including the button passed as `popUp`'s `in:` view, before `popUp` returns. The hide command then runs `close` (`MenusMac.swift:152`), which calls `cancelTracking()` and `makeFirstResponder` on that same stack. iOS exists to avoid this: `MenusIOS.swift:272` dismisses first and presses on the next turn so app code cannot destroy the presenting editor inside the selection callback. The comment at `MenusMac.swift:132` says the item action "has been sent by the next turn"; `choose` sends it immediately. `ChooserMacTests.testThePoppedUpMenuDispatchesTheChosenItem` (`ChooserMacTests.swift:153`) calls `performActionForItem` itself and stubs `onPress`, so a batch during tracking never runs.

   **Fix:** record the chosen id, mark `finished`, cancel tracking, and `press` only after `popUp` has returned and `valid` still holds. Mirror the iOS next-turn deferral.

3. **Should-fix** — `host/apple/Sources/ExactKit/Mac/MenusMac.swift:131`. On a real Mac the alertdialog chooser always opens below the invoker, so `open-above-sheet` cannot sit above the card.

   `popUp` is hardcoded at `(0, source.bounds.height + 2)` in a flipped view, which is below the button, for every `position-area`. The iOS sheet reads the row (`MenusIOS.swift:548`: `top*` permits `.down`, otherwise `.up`). The painted macOS layer reads it too (`MenusMac.swift:234`). Agent mode (D4) keeps every popover painted, so the evidence path in `llp/1021-menus.rfc.md:484` ("on iOS, macOS and in Chrome … bottom at the invoker's top") passes while a finger click on `open-above-sheet` (`app.contract:381`, `role="alertdialog"`, `position-area="top span-all"`) opens downward and can leave the screen. The macOS section (`llp/1021-menus.rfc.md:419`) states "popped up below" and does not record this the way the UIMenu limitation is recorded two paragraphs later.

   **Fix:** map the area onto the `popUp` point (a `top` area at the invoker's top edge, a centred area at `midX`), or document and test the NSMenu limitation and drop the claim that the Mac finger path opens above.

4. **Should-fix** — `contract/lower/src/menus.rs:126`. The compile-time refusal accepts a `link` action or cancel; both native hosts then refuse the popover.

   The match arms treat `link` like `button`. The refusal text at `menus.rs:117` and the hosts do not: actions and cancels are `isButton` (`ChooserMac.swift:63`, `MenusIOS.swift:475`), and `isButton` is `kind == "button"` (`Accessibility.swift:12`). A popover `role="alertdialog"` whose only row is `link press=… popovertarget=… popovertargetaction="hide"` compiles, then iOS logs `no action (a button with press)` and opens nothing, and macOS keeps the painted layer. No `menus.rs` integration test uses a `link`.

   **Fix:** classify a `link` row as `other` and refuse it with the same diagnostic as a `view`.

5. **Nit** — `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:551`. `position-area="center"` on an iOS alertdialog sheet stays on the arrowless `none` placement.

   `place` recenters `sourceRect` for every centred area, then returns immediately when the area is `center`, leaving `permittedArrowDirections = []` and `canOverlapSourceViewRect = true` as set just above. UIKit therefore keeps the arrowless slot. `PositionArea.origin` centers that value over the invoker, which is what the placement table promises (`llp/1021-menus.rfc.md:455`). An agent screenshot of a centered confirmation shows it over the button; the device shows it in the `none` slot.

   **Fix:** give `center` a defined sheet behavior (or refuse it on alertdialog popovers) and say so in the table.

6. **Nit** — `host/apple/tests/ExactKitTests/ChooserMacTests.swift:107` and `:145`. Two tests pass without exercising the claim.

   `testThePoppedUpMenuDispatchesTheChosenItem` injects `performActionForItem` and `cancelTracking` from `didBeginTracking` and replaces `onPress`, so it never lets AppKit deliver the item as `popUp` returns and never applies a batch. `testARowThatChangesItsTitleIsNeverChosenUnderTheOldOne` asserts `isOpen && !agentMode` is false, which is true for the whole run whenever `ExactEnv.agentMode` is set; the pop-up test in the same file skips that mode because the menu is never native there.

   **Fix:** drive a real item selection through `popUp` with the session `onPress` left in place, including a batch that unmounts the invoker, and skip or split the title test the way the pop-up test already skips agent mode.

## Verdict

DO NOT LAND.

## Round 2

**should-fix** — Authored popover margins never reach Apple placement. `host/apple/Sources/ExactKit/PositionArea.swift:18` (used at `MenusMac.swift:161`, `MenusMac.swift:248`, `MenusIOS.swift:326`).

`PositionArea.margins` reads `margin_top`, `margin_right`, `margin_bottom`, and `margin_left` from the node style dictionary, and the new placement (painted top layer, NSMenu `popUpPoint`, iOS lift) treats those points as the margin box. The Apple style encoder still drops all four rows in `presenter_ignores` (`host/apple/src/style.rs:52`). A production style batch therefore has no margin keys, and `NodeView.number` (`NodeViewMac.swift:786`) substitutes 0. `ChooserMacTests.testTheMenuPopsUpByItsPositionArea` and `PositionAreaTests` pass only because they write `margin_bottom: 12` through `wireBatch`, which skips that encoder. An authored `position-area: top; margin-bottom: 12px` sits flush against the invoker on macOS (menu and painted layer) and on the iOS painted lift. The web keeps the 12px: with `position-area`, only auto margins resolve to 0. A popover that leaves margin at `auto` still lands flush, because 0 is the right used value. Emit the four margins as resolved points when `position_area` is set; the dialog inset re-add at `style.rs:758` is the existing pattern. Keep `number()`'s fallback so a stray `"auto"` or percent still counts as 0.

2. **should-fix** — An inset side with no border-color is shaded from `#eeeeee`. `kernel/src/style/border.rs:37`.

`border_colors` is documented as resolving `currentcolor` against the node's computed color, and the solid arm does `color.unwrap_or(current)`. The inset arm substitutes `INSET_CURRENT` (`#eeeeee`, line 47) whenever the side names no border-color. CSS's initial `border-color` is `currentcolor`. A bare `hr` sets `color: gray` and `border-style: inset` and no border-color (`contract/lower/src/menus.rs:13`), so the `current` passed in is `rgb(128, 128, 128)` (`contract/cli/tests/it/menus.rs:81`). The same Chrome table maps `grey(0x80)` to top/left 44 and bottom/right 212, and `grey(0xee)` to 154/238. The hr test expects the `#eeeeee` pair (`menus.rs:83`) and states that the pair is independent of `color`. Current Blink `CalculateInsetOutsetColor`, called from `PaintSide` with `edge.GetColor()`, shades the computed border color. Current WebKit `BorderPainter::calculateBorderStyleColor` applies the same luminance thresholds to that color. Both shade the resolved color. Apple bakes the kernel pair into `border_color_*` (`host/apple/src/style.rs:738`) and Linux paints it (`host/linux/src/paint.rs:178`). The web host emits `border-style: inset` and the original color (`host/web/src/css.rs:488`), so Chrome shades gray to 44/212. A native `hr`, and any `border-style: inset` that only sets `color` (for example `color: red`), paints the `#eeeeee` shades. Shade `color.unwrap_or(current)`, and expect `[grey(44), grey(212), grey(212), grey(44)]` for the hr.

3. **should-fix** — A hidden button-menu row is still an enabled item. `host/apple/Sources/ExactKit/Mac/MenusMac.swift:326`.

`menu(of:)` sets `item.isEnabled = !row.disabled`. `display: none` hides the view in place and leaves it in `subviews` (`NodeViewMac.swift:84`). `shown()` (`MenusMac.swift:368`) runs only in the deferred `pick` guard (`MenusMac.swift:362`), which also requires `!inert`. The chooser path already dims that row: `choosable` includes `shown` (`ChooserMac.swift:64`) and that bit becomes `isEnabled` (`ChooserMac.swift:119`). A menu-shaped popover with a button row that is `display: none`, hidden, or inert still lists the row as enabled. Choosing it records a pick; the next turn drops the press. The item looks available and the click does nothing. A confirmation's hidden action is dimmed and cannot be chosen. Set `item.isEnabled = !row.disabled && !row.inert && shown(row, in: pop)`.

The deferred press holds for the races in the brief. A second item in one menu is stopped by `Confirmation.finished` and `Picked.taken`. `reset` cancels `choosing` and sets `picking.cancelled` before the turn. A batch that hides, retitles, disables, or replaces the row fails `valid`, `live` (`views[id] === node`), the title check, or `shown` on that turn. `ChooserMacTests` and `PopoverMacTests` drive a real `popUp`, including an unmount inside the press and a hide during tracking. `close(cancelling: false)` after `popUp` is what lets the recorded choice survive the menu ending; `close(cancelling: true)` clears a confirmation's `chosen`. `NSMenuItem.target` is weak, and `Choice` / `Pick` do not own `MenuHost`. The press goes by id after the identity check, on the turn after `didEndTracking`.

The argued `inset: auto` / `margin: 0` reset is unnecessary. In css-anchor-position-1, a `position-area` other than `none` resolves the used value of auto insets and auto margins to 0, and the default alignment for `top` / `top span-all` is `align-self: end` (flush with the anchor) and `justify-self: anchor-center`. The popover UA is `inset: 0; margin: auto`. The Chrome 154 boxes in LLP 1021 match that geometry: invoker x=190, width 172, so the centre is 276; a 242-wide popover lands at 155; y = 801 − 122 = 679. Explicit non-auto margins stay, which is finding 1.

Schema bit 176 (`position_area`) is the next bit after `order` at 175. `schema.json` has bits 0..=176 with no gap (177 rows). `from_bit` indexes `ALL[bit]`, `packed_table` asserts that density, and `STYLE_MASK_WORDS` is `len.div_ceil(64)`, still 3. `StyleId` is `repr(u8)`. The fixup updates the digest literal to `0x26c8_63a4_f45d_85f6` (`kernel/src/wire/codec.rs:656`) and the LLP bit number. That literal was not rehashed here. iOS `center` matches the documented compromise: the whole invoker, no arrow, overlap allowed, with UIKit choosing the vertical position.

LAND WITH FIXES
