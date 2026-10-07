# LLP 1104: The platform's controls by default — text fields and buttons

**Type:** RFC
**Status:** Draft r6, 2026-10-06. Not built, not yet reviewed in this form. r1–r4 (built: e97afa5af, aeb69b382, 3b904632d) gave fields compiled default rows and Exact-drawn focus rings. r5 replaced that design with the platform's controls. r6 builds on James's LLP 1069.011.001 r3 for the button and makes devolving CSS's rule, decided at compile time.
**Direction (Charlie, r5):** text fields and buttons default to the platform's look; hosts measure and draw it; author rows customise the native control where they can; hosts differ on purpose; the OS draws focus; fields and buttons share one switch whose default is `auto`.
**Systems:**
- Contract lowering (`contract/lower/src/fields.rs`, `controls.rs`, `tags.rs`): the default, the devolve rule, one `lower-appearance` error, the codemod.
- The kernel (`schema.json`'s `appearance` default per tag; `kernel/src/kernel/intrinsic.rs`; `layout.rs`'s `TextInput` measure; LLP 1069.011.001 D11's `ControlMeasurer`, extended to fields).
- The Apple hosts, iOS and tvOS on the UIKit presenter (`IOS/NodeViewIOS.swift`, `TextAreaIOS.swift`, `NativeButtonsIOS.swift`, `ControlsIOS.swift`) and macOS (`NodeViewMac.swift`, `TextAreaMac.swift`, `BoxLayerMac.swift`, `FieldEditingMac.swift`, `FocusMac.swift`, `NativeButtonsMac.swift`).
- The web host's reset (`host/web/index.html`, which both web targets ship) and the JS target.
- The painted hosts: Linux (`paint.rs`, `paint/control.rs`, `paint/caret.rs`), the terminal, Windows.
- Conformance, docs, the repo's apps.

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Builds on:** LLP 1069.011.001 r3 (James Ide: native buttons that look like the platform's own). A native button's face, its rows and its first-frame measure are that RFC's. This one makes the platform's control the default, adds text fields, and says when a control is the bare box instead.
**Amends:**
- LLP 1069.011: D1 (a button's default `appearance`), D4 (focus).
- LLP 1069.011.001: D12's refusals hold where the author wrote `appearance="auto"`; under the default, CSS's disabling rows make the bare box instead (D2).
- LLP 1102 §0's row for §3.15: restores "a field draws the platform's field".

**Related:** LLP 1069.001 (checkbox, radio, range, date: already `auto` by default, the model this follows); LLP 1069.011.000 (one press face); LLP 1075.003 §9.6 (iOS header search); LLP 1101.002 P7 (the terminal's field); LLP 1081 (the `-exact-` rule); CSS UI 4 §7.2.1 (properties disabling native appearance); `CLAUDE.md` ("the web is the standard")

## 1. Summary

A text field (`input` of a text type, `textarea`) and a `button` are the platform's own control unless the author says `appearance="none"`. The host builds the native control (`UITextField`, `NSTextField`, `UIButton`, `NSButton`, the browser's `<input>` and `<button>`), measures it before the first frame and draws it.

Which rows keep the control native is CSS's rule, not Exact's. A background, a border, a border radius or a border image written by the author makes a control the bare box (CSS UI 4 §7.2.1), and it does so the same way on every host, decided when the plan compiles. Every other row customises the native control as LLP 1069.011.001 maps it. Where a host can't express one, the agent reports a stand-in. Typography and colour stop at the control, as the browser's UA sheet makes them do. The OS draws focus.

This is CSS's own default. In the browser's UA stylesheet `input`, `textarea` and `button` are `appearance: auto`, and Exact's `all: unset` departs from it.

## 2. What exists

- **Fields** (r4, built):
  - Contract adds a sheet of rows under the author's: a 1px solid `light-dark(#c6c6c8, #48484a)` border, radius 6, padding 6/8, a fill and an ink (`fields.rs:114-127`).
  - The field is marked with `fieldStyle="default"` (prop 253). Disabled fields dim to opacity 0.5.
  - The native chrome is off on every host: iOS `borderStyle = .none` (`NodeViewIOS.swift:566-574`), macOS `isBezeled = false`, `focusRingType = .none` (`NodeViewMac.swift:768-783`), the web `all: unset` (`index.html:36`).
  - Focus rings are Exact's: the web restores the outline for `[data-fieldstyle]`, macOS draws a 2pt `keyboardFocusIndicatorColor` border in `boxPlan`, Linux draws `field_ring` in `accent-color`.
  - A field's font and colour inherit from its ancestors, on every host.
  - `appearance="auto"` on a field is refused (`lower-field-appearance`).
- **Buttons** (LLP 1069.011, 1069.011.000, built; 1069.011.001 r3, drafted):
  - A bare `button` is a `Pressable` with a fixed `appearance: none` row (`tags.rs:118-125`).
  - A literal `appearance="auto"` makes it a `Control` of type `button`. The host builds it and reports its size after the batch through `onIntrinsic` → `set_intrinsic_size`. Until then it lays out at `ControlKind::default_size` (`control.rs:66`), so frame two moves.
  - Its face is one `text` and/or one `image "symbol:…"`; `buttonStyle` picks one of twelve styles; D6 refuses every row outside a layout allowlist.
  - 1069.011.001 r3 (James) widens that: a subtitle and image placement, `gap`, `font-size`/`font-weight`, wrapping, `text-align`, `color`, `-exact-control-size`, `-exact-corner-style`, `border-radius` and `padding` as content fields, invokers, the symbol's own rows, disabled, `pointer-events`, and a first layout measured by the OS (D11). Background, border, shadow and material stay refused.
  - The node keeps focus and Exact draws the ring: a `CAShapeLayer` on iOS (`NodeViewIOS.swift:331-354`), AppKit's ring on the node on macOS, the browser's outline on the web, none on Linux.
  - On the web a native button is tiny until it is given a size (QUEUE, "Authoring-bench iOS round 13 polish").
- **Counts** (rough grep, classes not expanded):
  - Buttons: `apps/` 530 (5 native today; 416 style themselves inline, 18 by class only, 96 not at all); `contract/corpus` 93; `examples/` 27. About 100 have children other than one `text` and one `image`.
  - Fields: 43 in apps and 3 in the corpus already say `appearance="none"` (r4).

## 3. Decisions

### D1 — One switch, default `auto`, on fields and buttons alike

- **What the default covers:** `textarea` (except the Markdown editor); an `input` whose type is absent or one of `text`, `email`, `password`, `search`, `tel`, `url`, `number`; and `button`. `hidden`, `color`, `month` and `week` are unchanged.
- **The two values:** `auto` is the platform's control. `none` is the bare box Exact lays out and paints, as `button` is today and fields were before r4.
- **A literal:** the switch is resolved class then own attribute, as 1069.011 D1 has it. A bound or one-sided-class `appearance` is refused, and the error says to write `when` with two nodes. `lower-field-appearance` and `lower-button-appearance` become one error, `lower-appearance`.
- **Fixed rows:** the `button` tag's fixed `appearance: none` row goes. Fields gain no fixed rows: r4's sheet goes from lowering.
- **`buttonStyle`:** unchanged. With no style a native button is `bordered`. Fields get no style vocabulary in this RFC (§5 Q3).

### D2 — Devolving is CSS's rule, decided when the plan compiles

CSS UI 4 §7.2.1 names the properties that, written by the author, make a devolvable widget its primitive self: `background-*`, the `border-*` widths, styles and colours, `border-*-radius` and `border-image-*`. Exact follows that list on every host, and lowering applies it:

- **Under the default** (no `appearance` written), a field or button with any of those rows, from its own attributes or a resolved class, is the bare box: lowering writes `appearance: none` for it. That is what a browser draws for `<button style="background: …">`. No error and no lint: it is the web's behaviour, and the agent's `layout` reports it ("bare: `background-color`").
- **With `appearance="auto"` written**, those rows are refused (`lower-appearance`, naming the row), as LLP 1069.011.001 D12 refuses them. The author asked for the platform's control and for chrome the platform owns. One exception, declared: on a button, `border-radius` is UIKit's `cornerStyle = .fixed` (1069.011.001 D15), because UIKit names it as a content field and the style keeps its chrome. CSS would devolve it.
- **A disabling row present only under a condition** (a one-sided class, a `when` that adds it) would flip the control between native and bare at runtime. It is refused, and the error says to write `appearance="none"` or two nodes. A bound value on such a row (`background-color=(on ? a : b)`) is always present, so it devolves statically.
- **Content devolves too:** under the default, a button whose children don't fit 1069.011.001 D3's face (a third child, a non-symbol image, `svg`, nested boxes) is the bare box. With `appearance="auto"` written, they are refused (1069.011.001 D12).
- **The web's bare box is Exact's bare box.** Lowering's decision reaches the web as the same `appearance="none"`, so the reset (D8) applies. A browser's own devolved button keeps UA padding and a 2px outset border under the author's rows; Exact's bare box has neither, as `button` has had since 1069.011 D1. Declared deviation: one bare box on every host.

Because the decision is in the plan, it is one decision for every host. No host decides at runtime whether a control is native, and the kernel never learns it from a report.

### D3 — Other rows customise the native control

A row that doesn't devolve the control goes to it. The host applies it to the control, lays it out around the control (margin, size, position, transforms, opacity, as every host does today), or, where the platform can't express it, draws the nearest thing and reports a stand-in through the agent's `layout`. This is 1069.011.001's rule ("NSButton as is"), extended to fields.

**Buttons:** LLP 1069.011.001 D2–D9 and D13–D17, unchanged: `color`, `font-size`, `font-weight`, `white-space`, `line-clamp`, `text-align`, `gap`, `flex-direction`, `padding` as `contentInsets`, `border-radius` (D2's exception), `-exact-control-size`, `-exact-corner-style`, the symbol's `tint-color` and size, `accent-color` as the tint, invokers, disabled, `pointer-events`.

**Fields:**

| Row | iOS / tvOS (`UITextField .roundedRect`) | macOS (bezeled `NSTextField`) | Web |
|---|---|---|---|
| `color` | `textColor` | `textColor` | applies |
| `font-size`, `font-weight`, `font-family`, `letter-spacing` | `font`, `defaultTextAttributes` | `font` | applies |
| `text-align` | `textAlignment` | `alignment` | applies |
| `accent-color`, `caret-color` | `tintColor` | `insertionPointColor` (field editor) | applies |
| `padding-*` | text-rect insets added to the chrome's | added inside the bezel (cell subclass) | applies |
| `-exact-control-size` | stand-in (UIKit has no field sizes) | `controlSize` | CSS's absolute `font-size` keyword, as 1069.011.001 D8 |

The build fills each cell from a measured fixture (§4). A cell that doesn't hold becomes a reported stand-in, never a devolve.

- **Disabled:** the platform's own disabled look (`isEnabled = false`, the browser's `:disabled`). r4's opacity 0.5 row goes. An authored `color` is kept when disabled, as 1069.011.001 D16 and the browser do.
- **Placeholder:** the platform's own. iOS uses `UIColor.placeholderText` on the field and the textarea, which fixes the field's 30%-of-ink rule that disagreed with the textarea. macOS uses `placeholderAttributedString` with the system's colour, and the web uses the UA's (the `::placeholder` colour rule at `index.html:89` goes for native fields). An authored `::placeholder` colour is not in Contract and stays out of scope.

### D4 — Typography and colour stop at the control

The browser's UA sheet sets `font`, `color`, `letter-spacing`, `text-align` and friends on `input`, `textarea` and `button`, so an ancestor's typography does not reach them. LLP 1069.011.001 D2 makes a native button do the same. This RFC extends it to native fields:
- A native field's or button's font, colour, letter spacing and alignment come only from rows written on it (or, for a button, on its face's children, 1069.011.001 D2), each with its class.
- **Absent, the platform's:** the host leaves the control's own default (UIKit's and AppKit's field font, the label colour), so a future OS's default shows through. On the web it is the UA sheet's control font (13.33px in Chrome, `font: -webkit-small-control`), as a bare `<input>` has.
- **The kernel measures with what the host draws.** A field's text is still measured by the kernel (D5), so the host's measure answer includes the resolved default font for that control.
- **A bare box inherits, as today.** `appearance="none"`, written or by devolving, keeps Exact's reset: font and colour inherit. Declared deviation: a browser's devolved control still has the UA's font.

This changes what every unstyled field in the repo looks like on the web (§5 Q2).

### D5 — The host measures before the first frame

**Buttons:** LLP 1069.011.001 D11: a `ControlMeasurer` hook beside `TextMeasurer`, height-for-width, answered from a cache of real `UIButton.systemLayoutSizeFitting` (macOS `NSButton.fittingSize`) results. On a miss the presenter measures on the main thread before it presents that batch and asks for a relayout. A screen that adds no new face pays nothing, and nothing moves after the first frame.

**Fields** keep the kernel's text measure: `field-sizing`, `rows`, `MaxContent` width and the first baseline (`arena.rs:619-664`, `layout.rs:930-1037`). A field is not a `Control`: that path drops the baseline and can't follow a value whose size changes. The same hook answers, for a field, its **chrome**:
- **Insets:** four lengths the layout adds as it adds padding and border. The kernel keeps them apart from the `padding-*` and `border-*` rows.
- **Minimum height:** for a single-line field (UIKit's rounded-rect field is taller than its text).
- **Default font** (D4).

The answer depends only on the control kind, `-exact-control-size`, the content-size category, the legibility weight and the scale, not on the node, so the cache is a handful of entries per session. It is filled the way buttons fill theirs: on a miss, measured on the main thread before the batch is presented. No per-platform guess is written into the schema, and r5's `set_field_chrome` entry is not added.

- **Where the field's text goes:** the host places the text editor inside the chrome. The node's box is the control's frame. The content box is that frame minus the chrome insets and any authored padding.
- **Again:** a change of content-size category, legibility weight or `-exact-control-size` invalidates the entries it keys.
- **Web:** the browser lays out the native `<input>`, `<textarea>` and `<button>`, as it lays out a native button today.
- **Linux, the terminal, Windows:** answer synchronously from their own look (D7).

### D6 — The OS draws focus

- **The control is the focus owner.** For a field this is already true. For a native button it reverses 1069.011 D4: the native control becomes the first responder or focus candidate, and the node relays `focus`, `blur` and `key` from it. The agent's `tap` and focus operations address the node as today. The build proves that with `scripts/agent.mjs` on iOS and macOS, not by reading code.
- **iOS:** fields show the caret. Buttons use the focus system (`canBecomeFocused`, the system halo under a hardware keyboard and Full Keyboard Access). Bare pressables replace Exact's `CAShapeLayer` ring with `focusEffect = UIFocusHaloEffect(roundedRect:…)` from the node's shape.
- **tvOS:** the focus engine is the only input, and a focused `UIButton` lifts and highlights by itself. Native buttons and fields become focus candidates there as on iOS, and bare pressables keep their tvOS treatment as today. The tvOS build and its smoke are in this RFC's checks (`build.mjs --tvos`).
- **macOS:** fields use `focusRingType = .default`; textareas the scroll view's `drawFocusRingMask` while the text view is first responder; buttons `focusRingType = .default`, shown when `NSApp.isFullKeyboardAccessEnabled`, the OS's rule. Bare pressables keep AppKit's ring on the node. The `pressable` gap (a button relying on an ancestor's handler shows no ring, `NodeViewMac.swift:205`) is fixed in passing. r4's `fieldFocused` border in `boxPlan` and `showFieldFocus` go.
- **Web:** the UA's `:focus-visible` outline. The reset no longer reaches native controls, so `outline: auto` is the browser's own. The `[data-fieldstyle]` and `button:focus-visible` rules go.
- **Painted hosts:** no OS ring, so each paints one as part of its look (D7): `field_ring` for fields on Linux, and a new ring for buttons, which have none today.
- **Retired:** `fieldStyle` (prop 253) and its `data-fieldstyle` mark. Prop numbers aren't reused.

### D7 — The painted hosts have a look of their own

- **Linux** has no platform controls. Its painter draws a field and a button that read as one set, as it already draws a select or date frame (`paint/control.rs:86-126`). r4's colours, radius and padding become Linux's field look, painted by the host and not compiled into rows. The look is Linux's to tune. It answers D5's hook synchronously.
- **The terminal** keeps LLP 1101.002 P7's field (a fill and an ink), as the terminal host's look rather than compiled rows.
- **Windows** (`host/windows`, a painter today) takes Linux's look until it has native controls of its own.

### D8 — The web

- **The reset splits:** `all: unset` applies to `button`, `input` and `textarea` with `appearance="none"` (written or by D2, reaching the element as a data attribute) and no longer to native ones. Native ones are the browser's own `<input>`, `<textarea>` and `<button>`, with the authored rows as inline style.
- **Sizes are the browser's.** Conformance compares the JS target against the wasm oracle in Chrome, both with the UA's metrics.
- **Button looks:** `buttonStyle`'s web looks (`index.html:62-76`) apply as today; `bordered`, the default, is the UA button itself.
- **The size gap:** a native button on the web is tiny until it is given a size. Every unstyled button now hits it, so phase 1 fixes it.

### D9 — The repo's apps

- **Fields:** the 43 + 3 `appearance="none"` fields stay. A field with its own background or border now devolves by D2, so most could drop the attribute, and the codemod leaves them. Fields with no styling of their own take the platform's field. `visible_fields.rs` is rewritten around the default, D2 and the refusals.
- **Buttons:** D2 does most of the migration. A self-styled button that sets a background or border (most of the 434) is the bare box with no change to its source. A codemod adds `appearance="none"` to the rest that style themselves without one (colour, font or padding only: links, text actions), so they look as before. Its dry run prints the count. Bare unstyled buttons (about 96 + 79 + 4) take the platform's button. Converting a self-styled button to a customised native one is a per-app design decision, made app by app.
- **Docs:** both guides' control sections (`contract-for-agents.md:1605`, `contract-for-humans.md:949`), the pitfall about invisible fields, an `agent-pitfalls.md` entry for "a background makes it the bare box", and LLP 1102 §0's §3.15 row. Also the misattribution at LLP 1102:377, which r4's header repeated: LLP 1064 D6 is about `text-transform` and never said a field is a bare box. The bare box is LLP 1007's reset.

## 4. Verification

- **A fixture page** (`scripts/fixtures/native-controls.contract`, beside 1069.011.001's `native-buttons.contract`), screenshotted light and dark on iOS (iPhone 17 simulator), tvOS, macOS, the web and Linux:
  - a bare field and textarea; a password, search and number field; a disabled field;
  - a field with each D3 row on its own, and with each D2 row on its own (bare);
  - a button with each D2 row (bare), with rich children (bare), and with each `buttonStyle`;
  - `appearance="none"` on each, and an unstyled field and button inside a parent that sets font and colour (D4).
- **Lowering:** each D2 row devolves under the default and is refused with `appearance="auto"`; `border-radius` on a native button is admitted; a one-sided disabling row is refused; content devolves.
- **Native geometry:** each host's chrome answer per control size; the text rect and baseline of a single-line field inside the chrome; a `field-sizing: content` textarea growing with its value; a content-size-category change remeasuring; a filmed `screenshot … over 300 every 16` from launch with no frame moving after the first.
- **Focus:** Tab through fields and buttons with the OS ring on macOS (Keyboard navigation on and off) and the web; a hardware-keyboard halo on iPad; tvOS focus moving across a row of native buttons and a field; Linux's ring; no Exact ring drawn on Apple or the web.
- **Conformance:** the JS target against the oracle on the fixture.
- **Apps:** each app with changed controls is driven on iOS and the web (`scripts/agent.mjs`), screenshotted before and after.

## 5. Open questions

1. **iOS textarea.** UIKit has no bordered `UITextView`. Options: a hosted SwiftUI `TextField(axis: .vertical)` with `.roundedBorder`, the closest native multi-line field; or a `UITextView` with the system's separator stroke and the rounded field's radius, drawn by the host to match `UITextField`. Proposed: the second, because hosting SwiftUI for a text view brings its own focus and measurement paths.
2. **The web's field font.** D4 gives an unstyled native field the UA's control font on the web (13.33px in Chrome) and the platform's on Apple. It is what a bare `<input>` does, and it is the most visible change this RFC makes on the web. Proposed: keep it. An app that wants its own font writes it on the field or in a class.
3. **Field styles.** Should fields get a vocabulary like `buttonStyle` (macOS `borderShape` capsule, a plain borderless platform field)? Proposed: not now.
4. **`select`** is a `Control` already drawn natively. Should D2–D4 cover it? Proposed: yes, as a follow-up.

**Settled since r5:**
- r5 Q2 (codemod or not): D2 makes most self-styled buttons bare with no edit, and the codemod covers the rest.
- r5 Q3 (native chrome around arbitrary children): not planned. A button whose children don't fit the face is the bare box (D2). Hit testing, pressed dimming of Exact children and a title-less button's accessible name cost more than the look is worth.

## 6. Cost

| Part | Estimate |
|---|---|
| Lowering: the default, D2's devolve rule and refusals, one `lower-appearance`, r4's sheet and mark removed, the codemod, tests | a lane-day |
| Kernel: the field side of 1069.011.001's `ControlMeasurer` (chrome insets, minimum height, default font) | half a lane-day |
| Apple: native field chrome, D3's field mapping, the iOS textarea, D4's typography stop, focus moving to native controls, the halo, tvOS focus, ring code removed | two lane-days |
| Web: the reset split, the UA focus, the button-size gap, conformance | a lane-day |
| Painted hosts: Linux field and button looks, the button ring, the terminal, Windows | half a lane-day |
| Fixture, screenshots on five hosts, geometry checks, app drives, docs | a lane-day |

About six lane-days. 1069.011.001's build order steps 1–2 (the hook and iOS buttons) come first, and this RFC builds on them.

## 7. Revisions

- r1–r4, 2026-10-06: visible fields by compiled default rows (border, radius, padding, fill, ink), a literal `appearance="none"`, the conditional-class fallback, a `fieldStyle` mark for Exact-drawn focus rings, disabled dimming. Reviewed blind by Astra and Grok each round and built (e97afa5af, aeb69b382, 3b904632d). §2 records what that build left in the code.
- r5, 2026-10-06: Charlie disagreed with r1–r4's D1 ("we should default to the platform's look"): the platform's control by default for fields and buttons alike, hosts measure and draw, author rows customise the native control, hosts differ on purpose, the OS draws focus. Rewritten in place.
- r6, 2026-10-06: folds in James's LLP 1069.011.001 r3 (a button's face, rows and first-frame measure are its). Devolving follows CSS UI 4 §7.2.1's list and is decided in lowering for every host (D2), replacing r5's per-host runtime devolve and its lint. That makes most of the button codemod unnecessary. Typography and colour stop at a native control, as the UA sheet makes them (D4). Fields are measured before the first frame through the same hook, replacing r5's guessed chrome. tvOS and Windows are added, and r5's phase 2 is dropped.
