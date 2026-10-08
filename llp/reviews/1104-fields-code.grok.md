# Review: LLP 1104 steps 1–2 code (08e7e2ff4), 2026-10-07 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one session, at `08e7e2ff4` (`stopReason: end_turn`).
- **Method:** the same brief (sha256 `c35db0512a00178ee0c24ce305b2834462eeab1feda9c3b5979f5f0849795fc0`) behind the fixed preamble of tool rules (whole prompt sha256 `f614a6aa97fe5ec15f6b037c8b4d9d7087d9aae5b3e6c70b4d1bc9d6bb92d48e`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** findings 1–5 went to fix lanes F1–F3; 6 is deferred to step 3 (buttons), where the button font and control size land; see LLP 1104 §7, r9.

---

I'll audit the LLP 1104 field change on this checkout alone, starting from the lane commits and the binding design decisions.The audit is still open on macOS focus, the web box, and a few host paths. I’ll read those before writing findings.Focus still has a path on macOS. Next I’ll read the chrome miss, the web box, and the tests that claim to cover them.**Verdict:** LAND WITH FIXES

## Findings

### 1. Bare fields have no focus ring on the web, macOS, or Linux
**Severity:** material  
**Where:** `host/web/index.html:52`, `host/apple/Sources/ExactKit/Mac/NativeFieldsMac.swift:37`, `host/apple/Sources/ExactKit/Mac/FocusMac.swift:50`, `host/linux/src/paint.rs:1124`

D6 says the OS ring is only for the native control, and a bare field keeps Exact's ring. D2 turns any field with a background, border, or radius into that bare box with no source edit. Those fields used to carry `fieldStyle="default"`, which was the ring: the web rule `[data-fieldstyle]:focus-visible`, macOS `showFieldFocus`, and Linux `field_ring` on the outer box. All three are gone, and nothing replaced them for `appearance: none`.

Tab to `input background-color="#fff"` or `input appearance="none"`:

- Web: `button, input, textarea { all: unset }` removes the UA outline. The only restore left is `#exact-root button:focus-visible`. The field shows no outline.
- macOS: a non-native field is `focusRingType = .none`, and `focusRingMaskBounds` is `.zero` whenever `field != nil`, so the node ring does not draw either.
- Linux: `field_ring` runs only when the native chrome shape exists.

iOS still draws Exact's ring (`showFocusRing` bails out only for a native control), so the same field shows a ring on iOS and none on the other three. The caret still appears.

**Fix:** Give the bare editor its own ring again. On the web, `#exact-root input:not([data-native]):focus-visible, #exact-root textarea:not([data-native]):focus-visible { outline: revert; outline-offset: 2px }`. On macOS, draw the node ring for a bare field (the mask cannot stay empty just because an `NSTextField` exists) or set that field's `focusRingType` back to the system ring. On Linux, call `field_ring` on the outer box for a focused bare field as well.

### 2. A disabled native field on the web keeps the inherited text color
**Severity:** material  
**Where:** `host/web/index.html:39`

D3: an unauthored disabled field uses the browser's `:disabled` look; an authored `color` is kept. The native rule is author-origin `color: inherit` at `#exact-root [data-native]`. That beats the UA disabled color.

`column color="#111"` with `input disabled=true` and no color of its own: the text stays `#111`. A normal disabled input is gray. The disabled background can still come back through `all: revert`; the text does not.

**Fix:** `#exact-root [data-native]:disabled { color: revert }`. That selector outranks `color: inherit` and loses to an inline authored color, which is the split D3 describes.

### 3. On the web the kernel still inherits the rows D4 keeps at UA initials
**Severity:** material  
**Where:** `kernel/src/arena/control_text.rs:47`, `host/web/index.html:39`

D4: a native field on the web inherits font, line-height, letter-spacing, and color, and takes `text-align`, `text-indent`, `text-shadow`, and `text-transform` from the UA sheet. Those UA values are the kernel's initials, so the kernel and the browser agree. The stopped-row boundary is installed only when `Env` carries control styles. The web host never sets them, so `control_text_start` is absent and every inherited row comes from the parent. `all: revert` on `[data-native]` does the opposite in the page: `text-align` stays the UA `start`, indent and shadow stay none.

Parent `text-align=center text-indent=12` around a plain `input`: the browser shows start-aligned text with no indent. The kernel's computed style and text measure are centered and indented. The page is the one that matches D4.

**Fix:** On a native field, force `text-align`, `text-indent`, `text-shadow`, and `text-transform` to their initial values even when no control font is supplied. Leave font, line-height, letter-spacing, and color inheriting on that path.

### 4. Linux and the terminal never supply a control text style
**Severity:** material  
**Where:** `host/linux/tests/it/native_fields.rs:56`, `host/terminal/src/paint.rs:589`, `docs/contract-for-agents.md:1599`

D4 and D7: a painted host answers the control text style synchronously, and the guides say platforms other than the web use that style. Neither host calls `set_env` with one. The Linux test freezes the opposite: `control_text_styles` is `None`, and a native field inside `font-size=20 color="#33aa55"` computes 20px and that green.

The two hosts then diverge. Linux paints the inherited color. The terminal, when `text-color` is absent from the field's own mask, forces white on dark and black on light, so a parent color is dropped. A disabled Linux field is also the old half-opacity layer (`host/linux/src/paint.rs:933`), which fades the chrome, the text, and the focus ring together.

**Fix:** Before the first layout, pass a `ControlTextStyles` for the field and the textarea (the host font, `FieldText`, initials for the stopped rows), the same seam Apple uses, and point the Linux test at that. Paint disabled as a text/chrome treatment, not a 0.5 group opacity.

### 5. iOS throws away a provisional batch and still reports zero
**Severity:** minor  
**Where:** `host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:722`

`apply` returns immediately when `layoutProvisional` is set, before `presented(true)`. The batch's other ops are dropped, and `presentedProvisional` stays 0, which is the D5 check. macOS counts the flag and still applies the batch (`PresenterMac.swift:892`). `Host::layout` already refuses to return a batch that is still provisional after three passes, so this guard does not fire on the path that ships. If it ever did, iOS would lose the batch and the check would still read zero.

**Fix:** Count `presented(true)` and then drop only the geometry, or make `layoutProvisional` impossible on a batch that reaches `apply`.

### 6. One body font is stored for every control kind
**Severity:** minor  
**Where:** `host/apple/src/control_text.rs:74`, `kernel/src/control_measurement.rs:71`

D4 asks for one style per kind (field, secure field, textarea, button) and for `-exact-control-size`. `text_styles` copies a single `.body` / regular system font into `field`, `textarea`, and `button`. Secure and search correctly get their own chrome-cache kind; they do not get their own font. The button slot is unused in this step, and it is the field font rather than `UIButton`'s title font. The chrome request has no control-size axis; the comment defers it until that row exists.

**Fix:** Leave the field/textarea `.body` sharing as the iOS bullet specifies. When buttons land, fill `button` from a configured `UIButton`, and add control size to the chrome key with the row.

## What is right

Lowering matches D2: one decision after classes and shorthand expansion, any disabling longhand devolves, `appearance="auto"` refuses those rows, excluded types and the Markdown editor stay bare, buttons stay `appearance: none`, the compiled disabled opacity is gone, and prop 253 is retired and not reused.

Chrome sits outside the content box and outside authored padding. A single-line platform minimum is a min-height (it beats `max-height`, as CSS does). The single-line baseline is the centered line; a textarea stays top-aligned. `padding: 1em` resolves against the field's own font, and `set_env` re-derives and dirties the native fields.

On Apple, a chrome miss is measured on the main thread through `callMain`, the real answer is cached, and the provisional answer is not what `Host::layout` presents: it retries up to three times and refuses the batch. Retained regions publish the local content rect and allocate no field-rect storage when there are no fields. The web `data-native` rule steps native fields out of the reset, the placeholder rule does not reach them, and password/search stay distinct chrome kinds.