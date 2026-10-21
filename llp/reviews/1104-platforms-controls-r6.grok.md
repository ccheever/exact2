# Review: LLP 1104 r6, 2026-10-06 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one session, `--cwd` the worktree at `856ee5cdb` (`stopReason: end_turn`).
- **Method:** the same brief (sha256 `6044f668763f88d85fb01567af71e1bb0ea2989c043fdc0b87847f85e98c9520`) behind the fixed preamble of tool rules (whole prompt sha256 `3e42b25eda38ebb1f27daa06c3e504b006e57c9b26293fb3daee5d75a56cf6ae`); blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited; the first sentences are progress narration.
- **Verdict:** NOT READY.
- **Disposition:** folded into r7; see LLP 1104 §7.

---

I'll audit the r6 RFC against the code and the related LLPs it depends on, starting with the document and the files it cites.The RFC is in hand. Next I’ll check its code citations and the LLPs it amends against the tree.**Verdict:** NOT READY

## Findings

### 1. D2's devolve rule contradicts itself on the row a one-sided class already produces
**Severity:** blocking  
**Section:** D2

A one-sided class is not a separate input from a bound value. `class.rs` turns a row that only one arm of `class=(cond ? A : B)` sets into `cond ? value : none` (`contract/lower/src/class.rs:52-74`). D2 says that shape is refused ("a disabling row present only under a condition"), and also that any bound value on a disabling row "is always present, so it devolves statically." The same lowered row is both. `background-color=(on ? "red" : none)`, which authors can also write directly, has the same split.

That is the difference between a compile error and silently leaving the platform control forever, including on the side where the background is absent.

**Change:** Decide it once, on the lowered value. If any arm is `none` or absent, refuse and point at `when` with two elements. If every arm is a real value, devolve. Apply that same test to children (finding 6). Say the check runs on class rows after `class_rows`, not on the source attribute.

### 2. The field measure cannot do what D4 and D5 require
**Severity:** blocking  
**Section:** D4, D5

D5 caches chrome by control kind, `-exact-control-size`, content-size category, legibility weight, and scale, and says the answer "does not depend on the node." Minimum height does. A rounded-rect `UITextField` is taller than its text, and that height tracks the font actually drawn. An authored `font-size` / `font-weight` / `font-family` (`D3`'s table) is a different field from the default, and the kernel still measures the glyphs itself (`kernel/src/layout.rs:930-1037`, `kernel/src/arena.rs:628-664`).

D4 then says that when those rows are absent the host "leaves the control's own default," and that the kernel measures with what the host draws. Those are not the same font:

- The kernel's inherited field font is 16 (`kernel/tables/schema.json` `font_size` default 16; `arena.rs:543-546` copies inherited rows into `text_style`). Hosts already fall back to that 16 (`NodeViewIOS.swift:1227`, `NodeViewMac.swift:1091`).
- A stock `UITextField` font is the system 17pt and does not track Dynamic Type unless the host sets a text style and `adjustsFontForContentSizeCategory`. Putting content-size category in the cache key does not create that dependency.
- `NSTextField`'s default is the control-size font (about 13pt at regular), not 16.
- The web UA font is the 13.33px control font D4 cites. Q2 accepts that on the web and does not say how the wasm text engine reproduces it.

D5 also says the field is taller than its text and that layout keeps the first baseline, but not where the extra goes. UIKit vertically centers the text in a single-line field. The kernel's baseline is the top of the content box plus the ascent (`layout.rs:1030-1037`). A field beside a label on `align-items: baseline` will not match the pixels.

Box model is unspecified. A native button is fixed `border-box` (`contract/lower/src/controls.rs:471-474`). A field is content-box, and the r4 guide says an authored width is the content (`docs/contract-for-agents.md:1602-1603`). "The node's box is the control's frame" can mean either. Chrome insets added "as padding and border" change `width` one way under content-box and the other under border-box.

**Change:** Key the chrome answer by the font the control will draw (authored rows, otherwise the host font the control actually uses), and state that the iOS field font is the Dynamic Type body font rather than an untouched `UITextField` default. State that a single-line field centers its text in the chrome and that the baseline is that centered line. State that fields stay content-box, with chrome outside the authored width, or that they become border-box like buttons.

### 3. Moving focus to the native control drops rings and tab stops the current hosts deliberately keep
**Severity:** blocking  
**Section:** D6, D8

Three holes, all in the "OS draws focus" decision:

- **Web, bare controls.** `button:focus-visible` and `[data-fieldstyle]:focus-visible` exist to put the outline back after `all: unset` (`host/web/index.html:36, 48-50`). D6 deletes both rules and only says native controls keep the UA outline. A bare `button` or `appearance="none"` field still has `all: unset`, so it loses the ring it has today. The check "no Exact ring on the web" reads as success for that regression.
- **tvOS, two stops.** `NativeButtonIOS` is compiled for tvOS and sets `canBecomeFocused = false` so the node owns focus (`NativeButtonsIOS.swift:9-14, 44`). The node is a stop because `canBecomeFirstResponder` includes `isNativeButton` (`NodeViewIOS.swift:302`) and tvOS focus follows that (`RemoteTVOS.swift:20-24`). D6 makes the `UIButton` a candidate and also leaves "bare pressables" on today's path, but it never takes the node out of the focus engine. One button becomes two stops, and the system lift and `showFocusRing` (`NodeViewIOS.swift:331-354`, the tvOS branch) both draw.
- **macOS, keyboard navigation.** Today the node is the key view even when keyboard navigation is off (`FocusMac.swift:25-27`), and the `NSButton` refuses first responder (`NativeButtonsMac.swift:120`). D6 hands the button to AppKit and shows the ring only when `NSApp.isFullKeyboardAccessEnabled`. It amends 1069.011 D4's focus owner and does not say whether the "Tab reaches a button either way" rule survives. AppKit's own `NSButton` drops out of the key view loop when that setting is off.

The ancestor-handler gap is real (`pressable` at `NodeViewMac.swift:205`; the ring mask uses it at `FocusMac.swift:50`). "Fixed in passing" does not say that a `kind == "button"` with no handler of its own is still a ring.

**Change:** Keep a `:focus-visible` restore for `appearance="none"` only. On tvOS and iOS focus, the native control is the only focus target and the node is not. State that Tab still reaches buttons and fields when keyboard navigation is off, and that the OS rule is only whether the ring is visible. Say a button is ring-eligible without its own `press` handler.

### 4. "CSS UI 4 §7.2.1" is a broader list, and tripped by writing the property rather than by a non-initial value
**Severity:** material  
**Section:** D2, summary

§7.2.1 disables native appearance for author-origin values that are not the property's initial value. The longhands are `background-color`, `background-image`, the physical and logical `border-*-width` / `style` / `color`, the physical and logical `border-*-radius` longhands, and the `border-image-*` longhands. Not the rest of `background-*`.

Contract has `background-clip` and `background-attachment` (`kernel/tables/schema.json` bits 156 and 171) and no `border-image`. D2's `background-*` devolves a field that only sets `background-clip` or `background-attachment`. CSS does not.

"Written" is also the wrong trigger. `background-color: transparent`, `border-style: none`, `border-radius: 0`, and `background-image: none` are initial, so they do not devolve. `border-width: 0` does, because the CSS initial is `medium`. Messages already writes `background-color="#00000000"` and `border-width=0` on fields that are bare on purpose; a native field that sets a transparent background to avoid a fill would devolve under D2 and would not in a browser.

**Change:** Name the longhands Exact actually has. Devolve only when the used value is not that longhand's CSS initial (so `transparent`, `none`, and `0` radius do not). Keep the declared deviation that Exact's bare box has no UA padding and no 2px outset.

### 5. D4 does not stop the typography the UA sheet resets, and it adopts "buttons wrap"
**Severity:** material  
**Section:** D3, D4, D8

D4 stops font, colour, letter-spacing, and alignment. The sheet this repo already uses to imitate the UA reset is wider. `index.html:62` sets `font`, `color`, `letter-spacing: normal`, `word-spacing: normal`, `text-transform: none`, `text-shadow: none`, `font-variant: normal`, and `white-space: nowrap`. `line-height` and `font-style` are inherited (`schema.json` bits 66 and 69) and `line-height` changes the kernel's field height (`kernel/src/text.rs`). A parent `line-height: 2` or `font-style: italic` still reaches a native field. The fixture only puts font and colour on the parent, so it will not catch this.

`white-space` is the direct clash with 1069.011.001. D3 adopts that RFC's D5 unchanged: absent wrapping, and "a browser's `<button>` wraps too." D8 says `index.html:62-76` apply as today, and line 62 is `white-space: nowrap` plus truncation. Chrome, Firefox, and WebKit set `white-space: nowrap` on `button`. 1069.011 D6 and D8 already say the title is one line. r6 should not carry D5's browser sentence forward.

Colour is already not inherited on a dressed field. The r4 sheet writes `color` (`contract/lower/src/fields.rs:127`). The guide says the ink "does not inherit `color`" (`docs/contract-for-agents.md:1601`). §2's "font and colour inherit from its ancestors" is wrong for colour. Font does inherit. D4 is a font change, not a colour-inheritance change.

**Change:** List the rows that do not inherit on a native field or button, matching `index.html:62` plus `line-height` and `font-style`. For a native button, absent `white-space` is `nowrap` (truncate); wrapping is an authored `normal`. Say r6 narrows 1069.011.001 D5 on that point. Correct §2: the sheet sets ink; only the font inherits.

### 6. Conditional faces, and the codemod's idea of "styled," leave buttons that will change or fail to compile
**Severity:** material  
**Section:** D2, D9

D2 devolves or refuses children that don't fit 1069.011.001 D3's face, but only as a static shape ("a third child, a non-symbol image, `svg`, nested boxes"). Contract children include `when`, `match`, and `each` (`class.rs:123-128` walks them). 1069.011 D5 already walks `when` and `match` and refuses `each` for a native face. r6 does not say what happens when one branch fits and another does not, or when an image source is `symbol:` on one arm and a URL on the other. After class merge that is the same runtime flip D2 forbids for backgrounds. If the button is classified as native and the old grammar then refuses it, apps that compile today stop compiling. The codemod does not mark those `appearance="none"`.

The codemod also only keeps "colour, font or padding." A button whose only chrome is `text-align`, `gap`, `box-shadow`, or `flex-direction` is not a disabling row (finding 4) and is not on that list, so it becomes a platform button. `box-shadow` is not in §7.2.1; 1069.011.001 still refuses it when `appearance="auto"` is written. Under the default it would have to be a stand-in or a devolve, and D3 does not say which.

**Change:** If any reachable branch does not fit the face, devolve when `appearance` is absent and refuse when it is `auto`, same as finding 1. Extend the codemod to every button that is not a bare face plus layout rows: shadow, gap, alignment, and conditional images included. Print those in the dry run.

### 7. D3's field table replaces controls the hosts already specialise
**Severity:** material  
**Section:** D3, D1

The mapping is one `UITextField` with `.roundedRect` and one bezeled `NSTextField`. Password is already `isSecureTextEntry` (`NodeViewIOS.swift:1100`) and `NSSecureTextField` (`NodeViewMac.swift:768-769`). Email, URL, number, and search already set the keyboard (`NodeViewIOS.swift:1111-1118`). A reader of the table who builds "the" field can drop secure entry and still pass a screenshot of an empty box.

tvOS is in the same cell as iOS. `RemoteTVOS.swift` is a focus engine, not a text-field theme. Q1 discusses an iOS textarea and not a tvOS one. A rounded-rect iOS field is not the Apple TV text control.

**Change:** Keep the existing secure-entry, content-type, and keyboard mapping, and name them in the table. Give tvOS its own cell, or say the iOS rounded rect is an accepted stand-in there and the agent reports it.

### 8. The web reset split and the "tiny button" fix are unnamed
**Severity:** material  
**Section:** D8

`index.html:36` is one rule: `all: unset; display: block; box-sizing: content-box; cursor: default`. D8 moves `all: unset` off native controls and does not say whether `display: block` and `box-sizing` stay. Exact's bare node is block and content-box. The UA control is `inline-block`. Native buttons are separately `border-box` (`index.html:62`, `controls.rs:471-474`). Native fields are not.

The size gap is real (`QUEUE.md`: a native button is tiny on the web until given a width and height). D8 says phase 1 fixes it and does not say how. "Bordered is the UA button" and "the rules at `index.html:62-76` apply as today" disagree: line 62 already forces padding, nowrap, and flex centering on every `data-button-style`, including `bordered`.

**Change:** Keep `display: block` on native controls. Say a native field's box-sizing explicitly (finding 2). Say whether `bordered` drops the line-62 overrides and is measured from the UA button's intrinsic size, and make that the size-gap fix.

### 9. Verification will not catch the failures above, and six lane-days does not cover them
**Severity:** material  
**Section:** §4, §6

The lowering tests miss finding 1 (one-sided class, and a bound `none` arm) and finding 6 (`when` / `each` / conditional image). The parent font/colour fixture misses `line-height`, `white-space`, and `font-style`. Chrome geometry is "per control size," not per authored font, so finding 2's cache key passes. Focus checks "keyboard navigation on and off" without saying buttons must still be reached when it is off, and "no Exact ring on the web" blesses finding 3. Nothing drives a password field or a bare button's web ring.

Apple is "two lane-days" for field chrome, the textarea, typography, focus on three platforms, and tvOS. Focus ownership alone crosses `FocusMac.swift`, `NativeButtonsMac.swift`, `NativeButtonsIOS.swift`, and `RemoteTVOS.swift`, which already encode the opposite rule. The six days also assume 1069.011.001 steps 1–2 have landed. That RFC is unbuilt. Windows is not a separate painter: `host/windows/src/window.rs` uses `exact_linux::Presenter`, so it follows the Linux look with no extra paint work. Counting it as its own slice makes the painted-host half-day look tighter than it is, and does not pay for the Apple focus work.

**Change:** Add the cases in findings 1, 2, 3, 5, and 6 to §4, with the expected result for keyboard navigation off. Re-estimate Apple after the focus and font-measure rules are specified. Drop Windows as a separate paint task, and keep 1069.011.001's cost outside these six days only if that dependency is called out as unpaid.

### 10. Smaller citation and amend misses
**Severity:** minor  
**Section:** Systems, §2, D7, Amends

- Systems says `schema.json`'s "appearance default per tag." Appearance is one enum default, `auto` (`schema.json` `Appearance`, bit 146). The button's `none` is a fixed style on the tag (`tags.rs:118-125`). Removing that row is the whole mechanism. Fields need no schema change.
- "5 native today" is 4: two `appearance="auto"` buttons in `apps/messages/app.contract` and two `class=NativeGlass` (`apps/messages/shapes.contract:81-83`). The 530 / 93 / 27 button counts match a line count. `appearance="none"` on apps is 42, not 43.
- D7 changes 1101.002 P7 from a sheet chosen in the lowerer (`llp/1101.002-what-the-terminal-still-asks.rfc.md` §0 P7) to paint in the terminal host, and 1101.002 is not in Amends.
- `font-family` customises a field (D3) and is refused on a button (1069.011.001 D12). The summary's "every other row customises, or the agent reports a stand-in" does not match those refusals.
- "Windows (`host/windows`, a painter today)" overstates `host/windows`: it is the Linux presenter in a winit window.

**Change:** Point Systems at `tags.rs` and the enum default. Amend 1101.002 P7. Say button `font-family` stays a refusal. Count native buttons as 4.

## What is right

The direction is actually in the decisions: one `appearance` switch, default `auto`, by deleting the button's fixed `none` against the schema default `auto`; explicit `appearance="auto"` keeps 1069.011.001's refusals; `border-radius` on that path is a declared exception for D15. Compile time is the right place to choose, because a native button is a different node (`Control` versus `Pressable`).

The r4 citations that matter check out: the sheet at `fields.rs:114-127`, `fieldStyle` prop 253, `borderStyle = .none` (`NodeViewIOS.swift:566-574`), the bezel and `focusRingType = .none` (`NodeViewMac.swift:768-775`), `all: unset` (`index.html:36`), the 2pt macOS ring (`BoxLayerMac.swift:83-91`), Linux `field_ring`, and `ControlKind::default_size` of 64×34 (`kernel/src/control.rs:66-77`) reported later through `onIntrinsic`. LLP 1102:377 really does blame the bare box on LLP 1064, which is about `text-transform`. Keeping fields on the text-measure path is justified: the control path drops the baseline (`layout.rs:911-928`) and `set_intrinsic_size` rejects a `TextInput` (`kernel/src/kernel/intrinsic.rs:32-38`).