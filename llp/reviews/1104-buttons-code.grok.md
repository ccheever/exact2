# Review: LLP 1104 step 3 code (4e18bc622), 2026-10-07 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one session, at `4e18bc622` (`stopReason: end_turn`).
- **Method:** the same brief (sha256 `85a06a764a293bfc73bee373bdce5dd10b2766bca5dc1e23c359ac64def97c45`) behind the fixed preamble of tool rules (whole prompt sha256 `2c4fb6745811c6d56819415ddef53b99d1f82f59cebdfdabe03388251f207d50`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all ten findings went to fix lanes BF1 (kernel: 4, 5, 7), BF2 (Apple: 4, 6, 8, 10) and BF3 (web and Linux: 1, 2, 3, 9).

---

I'll audit the button-fidelity diff on this checkout alone, starting with the lane commits and the decisions they claim to implement.The lane is four commits over the kernel, Contract, and four hosts. I'll read the face, measure path, and each host's mapping against the decisions.The macOS focus path is the leading candidate. I’ll confirm the remaining open mappings—agent taps, control-text fonts, and the web clamp—before writing the verdict.**Verdict:** LAND WITH FIXES

## Findings

**1. On the web, an ancestor's `white-space` still reaches a native button**
Severity: blocking. `host/web/index.html:39`, `host/web/index.html:65`, `host/web/index.html:73`, `kernel/src/arena/control_text.rs:55-60`.

The kernel stop for a native button sets `white-space: normal` and `text-align: center`, so an ancestor does not supply those rows. The web rule `#exact-root [data-native] { all: revert }` does not. Chromium sets no `white-space` on `button`, so `revert` becomes `inherit`, and the title rule is `white-space: inherit`. A parent `white-space="nowrap"` with `button appearance="auto"` and a long title, and no `white-space` on the button: iOS wraps (D5, absent), and the web title stays one line. `text-align` reverts to the UA value `start`. A full-width button whose title wraps is centred on iOS (D6) and start-aligned on the web. `justify-content` only centres the grid track.

Set `white-space: normal` and `text-align: center` on `#exact-root button[data-button-style]`. Keep `white-space: inherit` on the title so an authored row on the button still wins through the inline style.

**2. Every web title is a line-clamp box, including `nowrap` and ordinary wrapping**
Severity: material. `host/web/index.html:73`, `host/web/src/element.rs:238-240`.

`button[data-button-style] > [data-exact-text]` is `display: -webkit-box !important` with `-webkit-line-clamp: var(--exact-button-clamp, unset)`. The variable is set only from the button's own `line-clamp`. `text-overflow: ellipsis` does not apply to a `-webkit-box`; the ellipsis comes from a positive clamp. `white-space="nowrap"` therefore has neither a clamp ellipsis nor a dependable text-overflow ellipsis (D5: one line, truncated at the end). An unset clamp is also the historical WebKit path that clips to one line, so the default wrap (the point of lifting the old `nowrap`) is not reliable. A `line-clamp` written on the title text is fine, because that child's inline `-webkit-line-clamp` beats the variable.

Apply `-webkit-box` and the clamp only when a positive clamp is in force on the button or the title. For `nowrap`, use `white-space: nowrap`, `overflow: hidden`, and `text-overflow: ellipsis` without `-webkit-box`.

**3. An icon-over-label button with no `gap` has no gap on the web**
Severity: material. `host/web/src/button_css.rs:39-40`, `host/web/index.html:72`.

D3's web stand-in for an absent gap is a space glyph. A row uses that glyph (`width: auto`). A column sets `--exact-button-space-height: var(--exact-button-row-gap, 0px)`, and the `::before` height falls back to `0px`. `flex-direction="column"` with an image and a title and no `gap` (the control tile) lays the symbol flush against the title. UIKit asks system spacing for that same face.

When `row-gap` is absent, give the column space the glyph's own line box (or `1em`), not `0px`.

**4. A button `em` is the field body font, and iOS then scales that size again**
Severity: material. `host/apple/src/control_text.rs:74-78`, `host/apple/Sources/ExactKit/IOS/FieldChromeIOS.swift:30`, `host/apple/Sources/ExactKit/IOS/FieldChromeIOS.swift:159-171`, `kernel/src/txn/relative.rs:61-70`, `host/apple/Sources/ExactKit/IOS/ButtonConfigurationIOS.swift:12-27`.

1104 D4: the button environment font is the title font `UIButton.Configuration` uses at that `-exact-control-size`, read from a configured button. The callback returns `UIFont.preferredFont(forTextStyle: .body)` (macOS: the regular system font) and copies that one font into field, textarea, and button. `font-size: 1em` on the button resolves against that already Dynamic-Type-scaled body size. The face then treats it as an authored size, and `font()` runs it through `UIFontMetrics(forTextStyle: .body).scaledFont` again. At the default content size the metrics scale is 1, so the fidelity check `font-size 23 == pointSize 23` cannot see it. At a larger size, `1em` is about body-points times the body scale, and a `large` control uses the same em base as `mini`. Unauthored titles are fine: the host leaves UIKit's font alone, and the Dynamic Type test only covers that path.

Return the configured button's title font per control size, and do not metrics-scale a length the kernel already resolved against that font.

**5. A subtitle makes `fits` true, and projections drop it**
Severity: material. `kernel/src/arena/button.rs:71`, `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:742-745`, `host/apple/Sources/ExactKit/Mac/MenusMac.swift:515-518`, `host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:48-50`, `host/apple/Sources/ExactKit/Accessibility.swift:222-226`.

`press_face` now sets `fits` for two texts and one image. `title(of:)` returns `face.shown` (`title ?? label`) whenever `fits`. A custom button whose children are `text "Save"` and `text "to iCloud"` used to join both strings; the menu item is now `Save`. A native tab with a symbol, a title, and a subtitle used to fail `fits` and stay authored; `TabBarFace` now accepts it and shows only the title. `segmentFace` does the same for a title and subtitle with no symbol.

Keep the second text out of `fits` for projections, or require `subtitle == nil` before projecting, and include the subtitle in the menu name when it is present.

**6. A finger tap does not light-dismiss a native button's popover on iOS**
Severity: material. `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:165`, `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:239-249`, `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:421-422`, `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:49-56`.

Native invokers skip the overlay `UIButton`. After the real press, `invokeNative` shows a non-confirmation popover with `agentShow`. `TopLayer.hitTest` returns nil outside the popover, so the tap falls through to the page. `agentTap`, which is the light-dismiss path, returns immediately for `isNativeButton`, and a real touch never calls it. Tapping the button again toggles it; tapping elsewhere leaves it open. Confirmations are unaffected (`openConfirmation`).

On a touch that is not inside the open popover and not its invoker, `drop` it, the same way `agentTap` does.

**7. Every layout dirties every native button and rebuilds its face**
Severity: material. `kernel/src/layout/buttons.rs:48-57`, `host/apple/src/control_text.rs:131-145`.

D11: a screen that adds no new face pays nothing. `prepare_buttons` calls `button_measure` and `mark_dirty` for every native button on every layout, including a cache hit whose `provisional` is false. Each call rebuilds face JSON. A later dirty elsewhere (a clock, another node) remeasures every native button and crosses into UIKit or AppKit again. The extra layout on a miss is withheld correctly; this cost is the hit path.

Dirty and remeasure only when the face, the rows, or the offered width are not already cached.

**8. macOS writes `.regular` when `-exact-control-size` is absent**
Severity: minor. `host/apple/Sources/ExactKit/Mac/ButtonConfigurationMac.swift:59-64`.

D1: an absent row is not written. The `default` arm assigns `controlSize = .regular`. That matches AppKit's default today, so nothing visible changes. `break` on the absent case, as iOS does for `buttonSize`.

**9. The Linux size test does not pin the new metrics**
Severity: minor. `host/linux/tests/it/native_buttons.rs:63-67`.

The comment says "Send in 13.33 px plus 2 × 12". The painter uses a 16 px medium font and padding 9. The assertion is `40 < width < 80`, which a wrong font or padding still passes. Assert the measured width and height against the painter's font, padding, and control-size scale.

**10. Percent and calc padding never become iOS insets**
Severity: minor. `host/apple/src/style.rs` (percent and calc are objects), `host/apple/Sources/ExactKit/IOS/ButtonConfigurationIOS.swift:106-110`.

Only `.number` is copied into `contentInsets`. A percent or calc padding is still resolved by Taffy and then subtracted back out, so the frame matches a button that kept the style's own insets. The authored padding is not on the control. Read the resolved point value the kernel already computed, or refuse a non-length padding on a native button.

## What is right

Bare stays the default; only `appearance="auto"` is native. The kernel face gives the child's title rows precedence, stops ancestor typography on Apple, and keeps `text-transform` for `tab` and `menuitem` projections. Lowering matches D12–D15 and D17: chrome and extra children are refused, `border-radius`, `padding`, and `pointer-events` are admitted, and `commandfor` is limited to the four command literals while a bound `popovertarget` is allowed. Apple measurement keys the real fitting size by face, width, content size, legibility, and scale; a miss returns that size marked provisional, the host withholds it, and the retry is a cache hit. The probe is a hidden view in the window, removed before return; `callMain` runs inline on main; the face bytes are copied before the hop; control text is primed before the first boot. iOS leaves an unauthored `buttonSize` and `cornerStyle` unset, maps padding as directional insets, and lets `border-radius` win. macOS reports the declared stand-ins. Linux measures synchronously and still paints non-button controls on the old path. iOS keeps the node as the focus owner; tvOS makes the `UIButton` the only stop. macOS still refuses first responder, which 1104 schedules as step 4.