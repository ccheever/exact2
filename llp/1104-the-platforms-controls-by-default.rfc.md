# LLP 1104: The platform's controls by default — text fields and buttons

**Type:** RFC
**Status:** Draft r9, 2026-10-07. Steps 1–2 (fields) built on `lane/1104-k` and under review fixes; steps 3–4 (buttons) not built. r1–r4 (built: e97afa5af, aeb69b382, 3b904632d) gave fields compiled default rows and Exact-drawn focus rings. r5 replaced that design with the platform's controls. r6 built on James's LLP 1069.011.001 r3. r6 was reviewed blind by Astra and Grok (both NOT READY); r7 folded both. r8 recorded Charlie's rulings and the iOS focus probe. r9 records what building fields settled.
**Direction (Charlie, r5):** text fields and buttons default to the platform's look; hosts measure and draw it; author rows customise the native control where they can; hosts differ on purpose; the OS draws focus; fields and buttons share one switch whose default is `auto`.
**Rulings (Charlie, on r7):**
- On the web, a native control inherits the page's font and colour, as the common reset does (D4). `AGENTS.md`'s "the web is the standard" is amended to match. The other platforms keep their own control fonts.
- D2's one static rule stands.
- The `border-radius` exception stands (§5 Q4, for James to confirm).
- The iOS native-button focus probe ran: UIKit can't take focus from Exact's traversal, so iOS keeps Exact's ring (D6, §5 settled).
- Charlie's lanes build LLP 1069.011.001 and this RFC as one program (§6).
**Systems:**
- Contract lowering (`contract/lower/src/controls.rs`, `fields.rs`, `tags.rs`, `class.rs`): the default, the admission rule, one `lower-appearance` error, the codemod.
- The kernel:
  - the `button` tag's fixed `appearance: none` row goes (`tags.rs:118-125`); `Appearance`'s schema default is already `auto`;
  - `Env` gains the controls' text styles (D4);
  - the non-inheriting rows on a native control (D4);
  - the single-line field's baseline (D5);
  - the field side of LLP 1069.011.001 D11's `ControlMeasurer` (D5).
- The Apple hosts:
  - iOS and tvOS on the UIKit presenter: `IOS/NodeViewIOS.swift`, `TextAreaIOS.swift`, `NativeButtonsIOS.swift`, `ControlsIOS.swift`, `FocusSearchIOS.swift`, `PresenterIOS.swift`, `RemoteTVOS.swift`;
  - macOS: `NodeViewMac.swift`, `TextAreaMac.swift`, `BoxLayerMac.swift`, `FieldEditingMac.swift`, `FocusMac.swift`, `NativeButtonsMac.swift`.
- The web host's stylesheet (`host/web/index.html`, which both web targets ship) and the JS target.
- The painted hosts: Linux (`paint.rs`, `paint/control.rs`, `paint/caret.rs`, `presenter/control.rs`), which Windows reuses, and the terminal (`host/terminal/src/paint.rs`).
- Conformance, docs, the repo's apps.

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Builds on:** LLP 1069.011.001 r3 (James Ide: native buttons that look like the platform's own). A native button's face, its rows and its first-frame measure are that RFC's. This one:
- makes the platform's control the default;
- adds text fields;
- says when a control is the bare box instead;
- moves focus to the native control.

This RFC's work starts after 1069.011.001's build order steps 1–5 (§6).
**Amends:**
- LLP 1069.011: D1 (a button's default `appearance`), D4 (focus).
- LLP 1069.011.001: under the default, a button its D12 would refuse is the bare box (D2).
- LLP 1101.002 P7: the terminal's field becomes the terminal host's look, not a sheet chosen in lowering.
- LLP 1102 §0's row for §3.15: restores "a field draws the platform's field".

**Related:**
- LLP 1069.001 (checkbox, radio, range, date: already `auto` by default, the model this follows)
- LLP 1069.011.000 (one press face)
- LLP 1075.003 §9.6 (iOS header search)
- LLP 1081 (the `-exact-` rule)
- CSS UI 4 §7.2.1 (properties disabling native appearance)
- Chromium's UA stylesheet (`third_party/blink/renderer/core/html/resources/html.css`)
- `CLAUDE.md` ("the web is the standard")

## 1. Summary

A text field (`input` of a text type, `textarea`) and a `button` are the platform's own control unless the author says `appearance="none"`. The host builds the native control (`UITextField`, `NSTextField`, `UIButton`, `NSButton`, the browser's `<input>`, `<textarea>` and `<button>`). It measures the control before the first frame and draws it.

**When a control is native.** Under the default, a control is native exactly when `appearance="auto"` would compile for it and the author wrote none of CSS's appearance-disabling rows. Otherwise it is the bare box. The decision is made once, in the plan, the same on every host.

**Typography and colour.** On Apple a native control's text starts from the platform's control font and colours rather than inheriting, and the host supplies that font to the kernel before layout. On the web a native control inherits the page's font and colour, as the common reset (normalize.css, Tailwind's preflight) makes it.

**Focus.** On a native control the OS draws focus. On the bare box Exact keeps the ring it draws today.

This is CSS's own default. In the browser's UA stylesheet `input`, `textarea` and `button` are `appearance: auto`, and Exact's `all: unset` departs from it.

## 2. What exists

- **Fields** (r4, built):
  - Contract adds a sheet of rows under the author's: a 1px solid `light-dark(#c6c6c8, #48484a)` border, radius 6, padding 6/8, a fill, and an ink in `color` (`fields.rs:114-127`).
  - A field's font inherits from its ancestors. Its colour does not, because the sheet writes it.
  - The field is marked with `fieldStyle="default"` (prop 253). Disabled fields dim to opacity 0.5.
  - The native chrome is off on every host: iOS `borderStyle = .none` (`NodeViewIOS.swift:566-574`), macOS `isBezeled = false` and `focusRingType = .none` (`NodeViewMac.swift:768-775`), the web `all: unset` (`index.html:36`).
  - Password fields are `isSecureTextEntry` (`NodeViewIOS.swift:1100`) and `NSSecureTextField` (`NodeViewMac.swift:768-769`). Email, URL, number and search set the keyboard (`NodeViewIOS.swift:1111-1118`).
  - Focus rings are Exact's: the web restores the outline for `[data-fieldstyle]`, macOS draws a 2pt `keyboardFocusIndicatorColor` border in `boxPlan` (`BoxLayerMac.swift:83-91`), Linux draws `field_ring`.
  - On tvOS a textarea is a read-only text view (`TextAreaIOS.swift:109`).
  - `appearance="auto"` on a field is refused (`lower-field-appearance`).
- **Buttons** (LLP 1069.011 and 1069.011.000 built; 1069.011.001 r3 drafted, not built):
  - A bare `button` is a `Pressable` with a fixed `appearance: none` row (`tags.rs:118-125`). `buttonStyle` on it is refused (`controls.rs:77-90`).
  - A literal `appearance="auto"` makes it a `Control` of type `button`, `border-box`, whose children are face data the kernel does not lay out (`node.rs:135`). The host reports its size after the batch through `onIntrinsic` → `set_intrinsic_size`. Until then it lays out at `ControlKind::default_size`, 64×34 (`control.rs:66-77`), so frame two moves.
  - Today: one `text` and/or one `symbol:` image; twelve `buttonStyle`s; every row outside a layout allowlist refused (1069.011 D6).
  - 1069.011.001 r3 adds a subtitle and image placement, title typography, wrapping, `color`, `-exact-control-size`, `-exact-corner-style`, `border-radius` and `padding` as content fields, invokers, the symbol's own rows, disabled, `pointer-events`, and a first layout measured by the OS (D11). D12 still refuses background, border, shadow, `filter`, `backdrop-filter`, `font-family`, `direction`, `press-scale`, and an image's size rows.
- **Focus:**
  - The node owns focus, and Exact draws the ring for a native button too.
  - **iOS:** Tab moves first responder and the presenter draws a ring (`PresenterIOS.swift:448`, `NodeViewIOS.swift:331-354`). UIKit's focus search skips `NodeView` and native buttons (`FocusSearchIOS.swift:31-36`).
  - **tvOS:** the node is the focus stop (`NodeViewIOS.swift:302`, `RemoteTVOS.swift:20-24`), and the `UIButton` says `canBecomeFocused = false` (`NativeButtonsIOS.swift:44`).
  - **macOS:** the node is the key view even when Keyboard navigation is off (`FocusMac.swift:22-27`), and the `NSButton` refuses first responder (`NativeButtonsMac.swift:120`).
  - **Web:** `button:focus-visible` puts the outline back after the reset (`index.html:48`).
  - **Linux:** no ring for buttons.
- **The terminal** paints no `Control` (`host/terminal/src/paint.rs:195`): a native button has no look there.
- **Windows** is the Linux presenter in a winit window (`host/windows/src/window.rs`).
- **The web's size gap:** a native button is tiny until it is given a size (QUEUE, "Authoring-bench iOS round 13 polish"). The cause is unmeasured.
- **Counts** (rough grep, classes not expanded):
  - Buttons: `apps/` 530, of which 4 are native today (two `appearance="auto"` and two through `class=NativeGlass`, all in Messages), 416 style themselves inline, 18 by class only and 96 not at all. `contract/corpus` has 93, `examples/` 27. About 100 have children other than one `text` and one `image`.
  - Fields: 42 in apps and 3 in the corpus say `appearance="none"`.

## 3. Decisions

### D1 — One switch, default `auto`, on fields and buttons alike

- **What the default covers:** `textarea` (except the Markdown editor); an `input` whose type is absent or one of `text`, `email`, `password`, `search`, `tel`, `url`, `number`; and `button`. Every other input type and the Markdown editor are unchanged, on every host and in the web stylesheet (D8).
- **The two values:** `auto` is the platform's control. `none` is the bare box Exact lays out and paints, as `button` is today and fields were before r4.
- **A literal:** the switch is resolved class then own attribute, as 1069.011 D1 has it. A bound or one-sided-class `appearance` is refused, and the error says to write `when` with two nodes. `lower-field-appearance` and `lower-button-appearance` become one error, `lower-appearance`.
- **Fixed rows:** the `button` tag's fixed `appearance: none` row goes. The schema default is already `auto`. Fields gain no fixed rows: r4's sheet goes from lowering.
- **`buttonStyle`:** with no style a native button is `bordered`. Fields get no style vocabulary in this RFC (§5 Q2).

### D2 — Which controls are native: one rule, decided in the plan

**CSS's disabling rows.** CSS UI 4 §7.2.1: a widget whose author origin has a cascaded value for any appearance-disabling property is devolved. Any value counts, initial ones included. The properties Contract has are:
- `background-color` and `background-image`;
- the `border-{top,right,bottom,left}-{width,style,color}` longhands;
- the four `border-*-radius` longhands;
- the shorthands that set them (`background`, `border`, `border-radius`, …).

`background-clip` and `background-attachment` are not on CSS's list. Contract has no `border-image`.

**The rule, in order:**
1. **`appearance="none"` written:** the bare box. `buttonStyle` is refused, as today.
2. **`appearance="auto"` written:** native. Lowering refuses, naming the row or child:
   - every disabling row;
   - everything LLP 1069.011.001 D12 refuses;
   - every attribute or context LLP 1069.011 and 1069.011.000 refuse on a native button;
   - children that don't fit 1069.011.001 D3's face (walked through `when` and `match` as 1069.011 D5 does; `each` refused).

   One declared exception: on a button, `border-radius` is UIKit's `cornerStyle = .fixed` (1069.011.001 D15). CSS would devolve it.
3. **Neither written (the default):** native exactly when rule 2 would admit the node **and** no disabling row is written, `border-radius` included. Otherwise the bare box. Under the default nothing is refused that compiles today.

**What counts as written** is decided on the node's rows after classes are merged (`class.rs`) and shorthands expanded. A row present on any arm counts: a one-sided class, a bound value with a `none` arm, a `when` that adds it. A face child that doesn't fit on any arm of a `when` or `match` counts too.

**Declared deviation:** CSS re-decides as rows come and go, so a conditional background flips a browser's button between native and devolved. Exact decides once, so that a node never changes type (`Control` against `Pressable`) at runtime. A control whose disabling row is present only sometimes is always the bare box, with or without that row's value.

**`buttonStyle` under the default** names a native button. On a node that rule 3 makes bare it is refused (`lower-button-style`). The error names the row or child that made it bare, and says to remove it or write `appearance="none"` without `buttonStyle`.

**Reported:** the agent's `layout` says "bare: `background-color`" (the first reason) for a default control that came out bare.

**The web's bare box is Exact's.** A browser's own devolved button keeps the UA padding and a 2px outset border under the author's rows. Exact's bare box has neither on any host, as `button` has had since 1069.011 D1. Declared deviation: one bare box everywhere.

### D3 — Other rows customise the native control

Rows rule 2 admits go to the control. The host does one of three things with each:
- **applies** it to the control;
- **lays it out** around the control: margin, size, position, transforms, opacity, as every host does today;
- **reports a stand-in** through the agent's `layout` where the platform can't express it, drawing the nearest thing. This is 1069.011.001's rule ("NSButton as is").

**Buttons:** LLP 1069.011.001 D2–D9 and D13–D17, unchanged. That includes D5: absent `white-space`, a button's title wraps, as UIKit's does and as Chromium's `button` does (its UA sheet sets no `white-space` on `button`). `font-family` stays refused (D12).

**Fields:**

| Row | iOS (`UITextField .roundedRect`) | macOS (bezeled `NSTextField`) | Web |
|---|---|---|---|
| `color` | `textColor` | `textColor` | applies |
| `font-size`, `font-weight`, `font-style`, `font-family` | `font` | `font` | applies |
| `letter-spacing` | `defaultTextAttributes[.kern]` | the value's `.kern` and the field editor's `typingAttributes`; a stand-in if that does not hold while editing | applies |
| `text-align` | `textAlignment` | `alignment` | applies |
| `accent-color`, `caret-color` | `tintColor` | `insertionPointColor` (field editor) | applies |
| `padding-*` | text-rect insets added to the chrome's | added inside the bezel (cell subclass) | applies |
| `-exact-control-size` | stand-in (UIKit has no field sizes) | `controlSize` | CSS's absolute `font-size` keyword, as 1069.011.001 D8 |

The table is a starting point. The build fills in each cell from a measured fixture (§4). A cell that doesn't hold becomes a reported stand-in, never a devolve.

- **What the hosts specialise today stays:** a password field is `isSecureTextEntry` / `NSSecureTextField`; email, URL, number and search keep their keyboards and content types. Each is in the fixture.
- **tvOS:** a field is UIKit's `UITextField` with tvOS's own look; editing opens the system's keyboard screen. Rows apply where tvOS's UIKit honours them, and stand-ins are reported. A textarea stays as today, a read-only text view; editable multi-line text on tvOS is out of scope.
- **Disabled:** the platform's own disabled look (`isEnabled = false`, the browser's `:disabled`). r4's opacity 0.5 row goes. An authored `color` is kept when disabled, as 1069.011.001 D16 and the browser do.
- **Placeholder:** the platform's own:
  - iOS uses `UIColor.placeholderText` on the field and the textarea. That fixes the field's 30%-of-ink rule, which disagreed with the textarea.
  - macOS uses `placeholderAttributedString` with the system's colour.
  - The web uses the UA's (the `::placeholder` rule at `index.html:89` stops reaching native fields).

  An authored `::placeholder` colour is not in Contract and stays out of scope.

### D4 — A native control's text: the platform's on Apple, the page's on the web

The browser's UA sheet gives `input`, `textarea` and `button` their own text style instead of the inherited one. In Chromium:
- `font: -webkit-small-control` (13.33px), `color: FieldText`;
- `letter-spacing: normal`, `word-spacing: normal`, `line-height: normal`;
- `text-transform: none`, `text-indent: 0`, `text-shadow: none`, `text-align: start`;
- and a `textarea` is `font-family: monospace`.

Almost every page an agent has seen undoes part of that with a reset:
- normalize.css: `font-family: inherit; font-size: 100%; line-height: 1.15`;
- Tailwind's preflight: `font: inherit; letter-spacing: inherit; color: inherit`.

Mobile Safari also zooms the page when a field under 16px is focused, which the raw default triggers on every tap.

**On the web** a native control takes the reset's path (Charlie, on r7; `AGENTS.md`, "The web is the standard"):
- **Inherited:** the font (family, size, weight, style, line height), `letter-spacing` and `color`. A textarea is not monospace.
- **From the UA sheet:** the rest, `word-spacing`, `text-transform`, `text-indent`, `text-shadow` and `text-align`. Their values are the kernel's own initial ones, so the kernel and the browser agree with no probe.
- **Declared deviation** from the raw UA sheet, chosen because it is the familiar path.

**On Apple** there is no reset convention, so a native control's text is the platform's:
- **The rows that stop:** the rows of the UA list above that Contract has. They don't inherit into a native field or button.
- **Starting values:** each starts from the control's value. An authored row (on the control, its class, or for a button its face's children, 1069.011.001 D2) replaces it, and clearing that row restores the control's value.
- **Alignment:** a native button's `text-align` keeps 1069.011.001 D6 (centred).

**A bare box inherits, as today, on every host.** `appearance="none"`, written or by D2, keeps Exact's reset.

**The Apple control text style is an environment fact.** The host supplies it to the kernel in `Env` before the first layout, as it supplies the safe-area insets. There is one style per control kind (field, secure field, textarea, button) and `-exact-control-size`. It is asked of the platform at runtime and never written down:
- **iOS:** fields and textareas use `UIFont.preferredFont(forTextStyle: .body)`, which follows Dynamic Type. The host sets `adjustsFontForContentSizeCategory`, so the font drawn is the font measured. The button's style is the title font `UIButton.Configuration` uses at that size, read from a configured button.
- **macOS:** `NSFont.systemFont(ofSize: NSFont.systemFontSize(for: controlSize))`, and the control's text colour.
- **Painted hosts:** their own look (D7).
- **The web hosts** supply none, which leaves the rows above inheriting. The wasm oracle's kernel and the JS target's page then agree with no probe.

Because the style is part of computed style:
- `em` lengths on the control resolve against it;
- the kernel's text measure uses it;
- the Apple hosts receive it as computed rows, as they receive inherited rows today (`host/apple/src/style.rs:823`), so they draw the font that was measured.

A change (Dynamic Type, legibility weight, control size) goes through `set_env`. It re-derives the styles that read it and invalidates their text and layout caches, as an `env()` length's change does.

### D5 — The host measures before the first frame

**Buttons:** LLP 1069.011.001 D11: a `ControlMeasurer` hook beside `TextMeasurer`, height-for-width. It is answered from a cache of real `UIButton.systemLayoutSizeFitting` (macOS `NSButton.fittingSize`) results. On a miss the presenter measures on the main thread before it presents that batch, and asks for a relayout.

**Fields** keep the kernel's text measure: `field-sizing`, `rows`, `MaxContent` width and the first baseline (`arena.rs:619-664`, `layout.rs:930-1037`). A field is not a `Control`, because that path drops the baseline (`layout.rs:911-928`) and can't follow a value whose size changes. The same hook answers, for a field, its chrome:
- **Insets:** four lengths the layout adds outside the content box, where padding and border go. They are kept apart from the `padding-*` and `border-*` rows.
- **Minimum height:** for a single-line field (UIKit's rounded-rect field is taller than its text).

**The box model is the UA's.**
- A native field is `content-box`: an authored `width` or `height` is the text area. The chrome insets and authored padding are added outside it.
- A native button is `border-box`, as today (`controls.rs:471-474`) and in Chromium.
- `min-*` and `max-*` apply as CSS says, to the same box.

**A single-line field centres its line.** When the used height exceeds the line (a minimum height, or an authored `height`), the text is centred vertically in the content box. UIKit, AppKit and browsers all do this. The field's baseline is that centred line's: content top + (content height − line height) / 2 + ascent. Today the kernel takes the top of the content box plus the ascent (`layout.rs:1030-1037`), and that changes for single-line native fields. A textarea's text starts at the top, as today.

**The cache key** for a field's chrome is:
- the control kind (secure and search fields included);
- `-exact-control-size`;
- the resolved font (family, size, weight);
- the content-size category, legibility weight and scale.

Phase 1 probes the chrome across fonts from 11 to 34pt, two families and constrained heights. The font comes out of the key only if the probes show the insets don't depend on it. A miss is measured on the main thread with a real configured control before the batch is presented, as for buttons. No per-platform guess is written into the schema.

- **Where the text goes:** the host places the text editor in the content box. The node's frame is the control's frame.
- **Web:** the browser lays out its own `<input>`, `<textarea>` and `<button>`.
- **Painted hosts:** answer synchronously from their look (D7).
- **No provisional frame:** each host counts batches it presented while any control's measure was provisional (`state.layout.provisional`). The checks assert zero (§4).

### D6 — The OS draws a native control's focus; Exact draws the bare box's

- **One focus owner per node.** A native field's owner is the native view. A native button's owner is the native view on macOS, tvOS and the web, and the node on iOS (below). Anything else's owner is the node. Every focus path uses that one mapping: Tab traversal, programmatic `focus()`/`blur()`, `autofocus`, `retainFocus`, collection pinning of a focused row, activation, and the routing of `focus`, `blur` and `key`. The node relays events from its owner. The agent's `tap` and focus operations address the node, as today.
- **A focusable button doesn't need a handler of its own.** A `button` is focusable and shows a ring with no press handler of its own. That fixes the `pressable` gap at `NodeViewMac.swift:205` and the ring mask at `FocusMac.swift:50`.
- **Native controls:**
  - **macOS:** the `NSButton` and the field accept first responder and stay in the key view loop whether or not Keyboard navigation is on, as the node does today (`FocusMac.swift:22-27`). The OS's setting doesn't decide whether Tab reaches a button. AppKit draws the ring (`focusRingType = .default`). A textarea's ring is the scroll view's `drawFocusRingMask` while the text view is first responder. r4's `fieldFocused` border in `boxPlan` and `showFieldFocus` go.
  - **iOS:** a field shows the caret. A native button keeps today's model (1069.011 D4): the node holds first responder, receives Tab, Return and Space, and Exact draws its ring, as a declared stand-in. A probe (2026-10-07, below) found UIKit can't take that focus:
    - **No focus system:** on iPad, UIKit's focus system exists only while a text input is first responder. `UIFocusSystem.focusSystem(for:)` is nil while an Exact node holds first responder.
    - **Requests are ignored:** `requestFocusUpdate(to:)`, `preferredFocusEnvironments` with `setNeedsFocusUpdate()`, and a forced `canBecomeFocused` never focused a `UIButton`.
    - **Keys go to the node:** Return and Space reached the node, never `.primaryActionTriggered`.
    - **Giving up first responder breaks the order:** UIKit's own Tab took over and skipped Exact's sequence.
    - **Full Keyboard Access owns Tab:** it moves and draws its own cursor, so the OS already draws focus there.

    The probe is a standalone UIKit app shaped like Exact's host, on an iPad Pro 11 simulator with iOS 27.0, driven by hardware-keyboard keys through XCUITest. One device check with a real keyboard stays owed before landing.
  - **tvOS:** the `UIButton` is the only focus stop. The node leaves the focus engine for a native button (`canBecomeFocused` false on the node, true on the button), so there are no double stops and no Exact ring over UIKit's lift.
  - **Web:** the UA's `:focus-visible` on the browser's own control.
- **The bare box keeps today's ring on every host:**
  - the web's restore (`button:focus-visible`, `index.html:48`) stays, scoped to the reset controls;
  - iOS keeps its first-responder ring;
  - tvOS and macOS keep theirs;
  - a bare field keeps Exact's ring, as a bare button does: on the web the `:focus-visible` restore covers bare `input` and `textarea`, and macOS and Linux ring the node. Before r9 this said a bare field draws its own focus; the code review showed that fields D2 makes bare had a ring under r4 and would lose it.

  This is what a browser does after `all: unset` with a focus restore (CSS UI 4 §7.2.2).
- **Painted hosts** have no OS ring and paint one as part of their look (D7): `field_ring` for fields on Linux, and a new ring for buttons there and in the terminal.
- **Retired:** `fieldStyle` (prop 253) and its `data-fieldstyle` mark. Prop numbers aren't reused.

### D7 — The painted hosts have a look of their own

- **Linux** has no platform controls. Its painter draws a field and a button that read as one set, as it already draws a select or date frame (`paint/control.rs:86-126`). r4's colours, radius and padding become Linux's field look, painted by the host rather than compiled into rows, and Linux's to tune. It answers D5's hook and D4's text style synchronously.
- **Windows** is the Linux presenter (`host/windows/src/window.rs`) and gets Linux's look with no work of its own.
- **The terminal:**
  - Its field keeps LLP 1101.002 P7's look (a fill and an ink), as the host's paint rather than compiled rows. 1101.002 P7 is amended.
  - A native button gets a look and a measure, which it has none of today. The terminal paints the face's title from the button's face data, since the kernel does not lay out a `Control`'s children (`node.rs:135`). The look is reverse video with one cell of padding each side, and the symbol is dropped. A focused button is drawn in bold.
  - It answers `ControlMeasurer` in cells. Enter and Space activate the button.
  - The terminal's apps (Todo, Harness) are driven before and after.

### D8 — The web

- **The reset stays and native controls step out of it.** `button, input, textarea { all: unset; … }` (`index.html:36`) stays as it is, so excluded input types, the Markdown editor (which starts as a `textarea`, `markup-editor.js:99`) and every bare control are unchanged.
  - **The marker:** lowering marks a native control in D1's domain (`data-native`).
  - **The new rule:** `#exact-root [data-native] { all: revert; display: block; font: inherit; letter-spacing: inherit; color: inherit; }` gives the control back the UA sheet with the reset's typography (D4), keeping Exact's block display.
  - **The box:** `box-sizing` is the UA's, content-box for fields and border-box for buttons (D5).
  - **The authored rows** arrive as inline style, as today.
- **Button looks:** `buttonStyle`'s web looks (`index.html:62-76`) are 1069.011.001's to change in its step 3. Its D5, for one, lifts line 62's `nowrap`. `bordered`, the default, is the UA button: line 62's `revert` rows already give it the UA's padding, border and font.
- **The size gap:** the cause of the tiny native button is unmeasured. Phase 1 traces it first, with the JS target and the wasm oracle side by side, and fixes it before any app's buttons turn native.
- **Sizes are the browser's.** Conformance compares the JS target against the wasm oracle in Chrome, and both pages ship this stylesheet. So §4 also compares them with a plain page of the same elements and no Exact stylesheet.

### D9 — The repo's apps

- **Fields:**
  - The 42 + 3 `appearance="none"` fields stay.
  - A field with its own background or border is the bare box by D2 with no edit. The codemod leaves its `appearance="none"` in place.
  - Fields with no disabling rows take the platform's field. `visible_fields.rs` is rewritten around D2's rule and refusals.
- **Buttons:**
  - D2 does most of the migration. A button with a background, a border, a radius, non-face children, or any row 1069.011.001 D12 refuses is the bare box with no change to its source.
  - A codemod adds `appearance="none"` to every button that rule 3 would make native and that has any row beyond layout rows (margin, size, position, flex item rows), so it looks as before: colour, font, padding, `gap`, `text-align` or `flex-direction`. Its dry run prints the list and the count.
  - Bare unstyled buttons (about 96 + 79 + 4) take the platform's button.
  - Converting a self-styled button to a customised native one is a per-app design decision, made app by app.
- **Docs:**
  - Both guides' control sections (`contract-for-agents.md:1597-1608`, `contract-for-humans.md:946-950`).
  - The pitfall about invisible fields.
  - A new `agent-pitfalls.md` entry: "a background, border or radius makes a button the bare box; `buttonStyle` then refuses and says why".
  - LLP 1102 §0's §3.15 row.
  - The misattribution at LLP 1102:377, which r4's header repeated: LLP 1064 D6 is about `text-transform` and never said a field is a bare box. The bare box is LLP 1007's reset.

## 4. Verification

- **A fixture page** (`scripts/fixtures/native-controls.contract`, beside 1069.011.001's `native-buttons.contract`), screenshotted light and dark on iOS (iPhone 17 simulator), tvOS, macOS, the web and Linux, and printed by the terminal. It holds:
  - a bare field and textarea; password, search, email and number fields; a disabled field;
  - a field with each D3 row on its own;
  - a field and a button with each disabling row on its own (bare);
  - a button with each `buttonStyle`, with rich children (bare), and with a conditional background (bare on both arms);
  - `appearance="none"` on each;
  - an unstyled field and button inside a parent that sets `font-size`, `font-style: italic`, `line-height: 2`, `letter-spacing` and `color`: inherited on the web, the platform's on Apple (D4);
  - a native field with `padding="1em"` (D4's `em`).
- **Against the platform itself:**
  - each native field and button beside a `UITextField`, `NSTextField`, `UIButton` and `NSButton` configured by hand in Swift for the same intent, pixel-diffed, as 1069.011.001 §4 does for buttons;
  - on the web, the same elements in a plain page without Exact's stylesheet.
- **Lowering tests:** D2's rule in order for each disabling row and each 1069.011.001 D12 refusal, under `none`, `auto` and the default. Also:
  - a one-sided class, a bound `none` arm and a `when`-added row (bare under the default, refused under `auto`);
  - `border-radius` admitted on an `auto` button and bare under the default;
  - a face child on one arm of a `when`;
  - `buttonStyle` on a node the default made bare, refused with its reason.
- **Native geometry:**
  - each host's chrome per control size and per font across the probe range;
  - the baseline of a single-line field beside a `text` under `align-items: baseline` with an authored height;
  - a `field-sizing: content` textarea growing with its value;
  - a content-size-category change remeasuring and re-deriving `em` lengths.
- **No provisional frame:** `state.layout.provisional` is zero after a cold launch with an empty cache, and after a screen that adds a new control kind and font after launch.
- **Focus**, each with keyboard navigation on and off:
  - Tab reaches every native and bare field and button on macOS and the web;
  - the OS ring shows on native controls and Exact's ring on bare ones;
  - `tabindex`, programmatic `focus()`/`blur()`, `retainFocus`, removing a focused control, and a cancelled key activation each run exactly once;
  - a hardware-keyboard Tab on a physical iPad across native and bare buttons and fields, with Exact's ring on each (D6), and Full Keyboard Access reaching them;
  - tvOS focus moves across a row of native buttons and a field, one stop each;
  - Linux's and the terminal's rings.
- **Excluded controls:** checkbox, radio, range, file and date inputs, and the Markdown editor (load, edit, source mode), unchanged before and after on the web.
- **Conformance:** the JS target against the oracle on the fixture.
- **Apps:** each app with changed controls is driven on iOS and the web (`scripts/agent.mjs`), screenshotted before and after the codemod. The terminal drives Todo and Harness.

## 5. Open questions

1. **iOS textarea.** UIKit has no bordered `UITextView`. Options:
   - a hosted SwiftUI `TextField(axis: .vertical)` with `.roundedBorder`, the closest native multi-line field;
   - a `UITextView` with the system's separator stroke and the rounded field's radius, drawn by the host to match `UITextField`.

   Proposed: the second, because hosting SwiftUI for a text view brings its own focus and measurement paths. Apple's own apps rarely border multi-line text, so this is the field's look extended, not a platform control.
2. **Field styles.** Should fields get a vocabulary like `buttonStyle` (macOS `borderShape` capsule, a plain borderless platform field)? Proposed: not now.
3. **`select`** is a `Control` already drawn natively. Should D2–D4 cover it? Proposed: yes, as a follow-up.
4. **The `border-radius` exception** (D2 rule 2) is James's D15. It keeps Lexy's rounded native buttons, at the price of one CSS deviation under `appearance="auto"`. Charlie agrees; James to confirm.

**Settled:**
- r5 Q2: D2 makes most self-styled buttons bare with no edit, and the codemod covers the rest.
- r5 Q3: native chrome around arbitrary children is not planned (D2).
- r7 Q2: the web's control font. The page's, as the reset convention has it (D4, Charlie).
- r7 Q3: iOS native-button focus. The probe found UIKit can't take focus from Exact's traversal, so iOS keeps Exact's ring as a stand-in (D6).

## 6. Cost and order

Charlie's lanes build LLP 1069.011.001 and this RFC as one program. The order lets fields, the original problem, ship first:
1. **The shared measure hook.** `ControlMeasurer` with a host that answers "unknown" (1069.011.001 step 1's kernel half) and `Env`'s control text styles (D4).
2. **Fields:**
   - D1 and D2 for fields;
   - D3's field mapping and D4 on fields;
   - D5's field chrome on the hook;
   - D6 for fields;
   - D8's web rule for fields;
   - the painted hosts' field looks;
   - fields in D9.

   This needs nothing else from 1069.011.001.
3. **Native buttons:** 1069.011.001 steps 2–5 (the iOS mappings and cache, web and Linux, macOS, invokers and the rest).
4. **Buttons by default:** D1 and D2 for buttons, the codemod, button focus (D6), the terminal's button, and the rest of D9.

| Part | Estimate |
|---|---|
| Lowering: the default, D2's ordered rule, one `lower-appearance`, `data-native`, r4's sheet and mark removed, the codemod and its dry run, tests | a lane-day |
| Kernel: control text styles in `Env` and the non-inheriting rows, invalidation, the single-line baseline, the field side of `ControlMeasurer`, the provisional counter | a lane-day and a half |
| Apple: native field chrome and the D3 mapping, secure and typed fields, the iOS textarea, D4's text style from the OS, focus ownership on tvOS and macOS (iOS stays as today), ring code removed | three lane-days |
| Web: the `data-native` rule, the size-gap trace and fix, conformance and the plain-page comparison | a lane-day |
| Painted hosts: Linux field and button looks and the button ring; the terminal's button look, measure and keys | a lane-day |
| Fixture, hand-configured comparisons, screenshots on five hosts, geometry, focus and app drives, docs | a lane-day and a half |

About nine lane-days for this RFC. LLP 1069.011.001 is unestimated in its text; its five steps look like another five to seven. So the program is about fifteen lane-days, with fields usable after steps 1–2 (about five).

## 7. Revisions

- r1–r4, 2026-10-06: visible fields by compiled default rows (border, radius, padding, fill, ink), a literal `appearance="none"`, the conditional-class fallback, a `fieldStyle` mark for Exact-drawn focus rings, disabled dimming. Reviewed blind by Astra and Grok each round and built (e97afa5af, aeb69b382, 3b904632d). §2 records what that build left in the code.
- r5, 2026-10-06: Charlie disagreed with r1–r4's D1 ("we should default to the platform's look"): the platform's control by default for fields and buttons alike, hosts measure and draw, author rows customise the native control, hosts differ on purpose, the OS draws focus. Rewritten in place.
- r6, 2026-10-06: built on James's LLP 1069.011.001 r3. Devolving follows CSS UI 4 §7.2.1, decided in lowering. Typography stops at a native control. Fields measured through the same hook. tvOS and Windows added, and r5's phase 2 dropped.
- r7, 2026-10-06: folds the blind reviews of r6 by Astra max (`llp/reviews/1104-platforms-controls-r6.astra.md`) and Grok 4.7 xhigh (`…r6.grok.md`), both NOT READY. Disposition:
  - **Accepted, D2:** the conditional-devolve contradiction (Astra 2, Grok 1) and D3 against 1069.011.001 D12 (Astra 3). D2 is now one ordered rule: under the default, native when `auto` would compile and no disabling row is written. A row on any arm counts, and nothing that compiles today is refused. `buttonStyle` on a node that came out bare is refused with its reason.
  - **Accepted, D2 and D3:** CSS's exact list (Astra 11, Grok 4), with `background-clip` and `-attachment` out. Conditional faces (Grok 6).
  - **Accepted, D3:** secure and typed fields, tvOS's own cell, macOS `letter-spacing` (Grok 7, Astra 11).
  - **Accepted, D4:** the control's text style enters computed style through `Env` before layout, with `em`, invalidation and the full UA list including `line-height` and `font-style` (Astra 1, Grok 2 and 5).
  - **Accepted, D5:** the box model, the centred single-line baseline, and a chrome key that includes the font until probes say otherwise (Astra 4, Grok 2).
  - **Accepted, D6:** one focus owner, the web restore kept for bare controls, tvOS single stops, macOS Tab regardless of Keyboard navigation, ring eligibility without a handler (Astra 5 and 6, Grok 3). r6's iOS halo for bare pressables is dropped: a halo needs a UIKit focus item. Bare controls keep Exact's ring everywhere, and iOS native-button focus is a probe (§5 Q3).
  - **Accepted, D7:** the terminal's button and tvOS's textarea (Astra 8). Windows is the Linux presenter (Grok 10).
  - **Accepted, D8:** the reset is scoped by `data-native` and `all: revert`, leaving excluded controls and the Markdown editor alone (Astra 7, Grok 8).
  - **Accepted, §4 and §6:** a provisional-frame counter instead of a film, the hand-configured and plain-page comparisons, the missing cases (Astra 9, Grok 9). The cost is incremental to 1069.011.001 steps 1–5 and re-estimated (Astra 10, Grok 9).
  - **Accepted, minor (Grok 10, Astra 11):** counts (4 native, 42 `none`), the schema line, 1101.002 P7 amended, §2's colour premise corrected.
  - **Not taken, Grok 4's trigger:** Grok said only a non-initial value devolves. CSS UI 4 §7.2.1 says a cascaded author value devolves, any value. Messages' transparent-background fields already say `appearance="none"`.
  - **Not taken, Grok 5's `nowrap`:** Grok said browsers make a button `nowrap`. Chromium's UA sheet sets no `white-space` on `button` (only `pre` on `input[type=button]`), so 1069.011.001 D5's wrapping stands. `index.html:62`'s `nowrap` is Exact's own, from 1069.011, and 1069.011.001 lifts it.
- r8, 2026-10-06: Charlie's rulings on r7.
  - D4: on the web a native control inherits the page's font and colour, as the common reset does, and `AGENTS.md`'s rule is amended to allow it. Apple keeps the platform's control text style through `Env`.
  - D2 and the `border-radius` exception stand.
  - The iOS focus probe ran (2026-10-07): UIKit's focus system can't be driven from Exact's traversal, so a native iOS button keeps the node as focus owner and Exact's ring as a stand-in (D6).
  - One program builds 1069.011.001 and this RFC, fields first (§6).
- r9, 2026-10-07: what building steps 1–2 settled.
  - **Built:** lanes K (kernel), L (lowering), W (web), P (Linux, terminal), A1 (iOS, tvOS), A2 (macOS), K2 and K3 (field content rects through retained regions, kept out of every region frame), merged on `lane/1104-k` and verified (3,371 root tests; Apple, web, Linux, terminal suites; conformance 102 of 102; zero provisional frames presented on a cold launch on iOS and macOS).
  - **The hook:** D5's chrome hook is `TextMeasurer::field_chrome` (a defaulted method), not a separate `ControlMeasurer`; buttons' measure (1069.011.001 D11) joins it at step 3.
  - **Platform facts found:** a default `NSTextField` on macOS 27.0.1 reports `.squareBezel`, which macOS uses. UIKit exposes no rounded-field bezel values, so the iOS textarea's border is a 5pt radius and a 0.5pt separator stroke, matched against a real field.
  - **D6:** a bare field keeps Exact's ring (above).
  - **Painted hosts:** Linux and the terminal supply their own control text style (D4, D7). An orchestration call to let them inherit like the web was reversed after both code reviews showed white text on Linux's white field.
  - **Code reviews** of the merged build by Astra max (`llp/reviews/1104-fields-code.astra.md`, DO NOT LAND, 8 findings) and Grok 4.7 xhigh (`…code.grok.md`, LAND WITH FIXES, 6 findings): all but Grok 6 (one font for every control kind, deferred to step 3) went to three fix lanes.
