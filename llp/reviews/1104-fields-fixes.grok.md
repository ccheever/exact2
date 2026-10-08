# Review: LLP 1104 steps 1–2 fix delta (aa2f4e057..ed36af6d0), 2026-10-07 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one session, at `ed36af6d0` (`stopReason: end_turn`).
- **Method:** the same delta brief (sha256 `16419c08d08f917ff1f124b51e71bc65dfd3b20ee22e99a26addb4f12e15b12d`) behind the fixed preamble of tool rules (whole prompt sha256 `646d35de6e9b7848ad11a32dd28f6407e808cef1ccb050c7e109de38eb159855`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** new findings 1–3 (the same three Astra found) went to fix lane F4 (b8ba774dc). On Grok 5 the families disagreed (fixed here; Astra: provisional geometry now reaches the screen); F4 took Astra's reading and withholds provisional geometry at the publication boundary.

---

I'll audit the fix delta against the two earlier reports, starting with those findings and the commit range.The earlier reports are in. Next I’ll read the fix diff against each finding and check whether the new tests actually hit the failure.The diff shows the intended fixes. I’ll read the changed paths next to see whether each one actually closes the old failure and whether it adds a new one.The audit is complete except for the two new-bug sites. I'll confirm those line numbers, then write the findings.**Verdict:** LAND WITH FIXES

**Earlier findings:**

- Astra 1 — fixed; test `host/apple/tests/ExactKitTests/NativeFieldsIOSTests.swift:15` boots and replaces a fieldless plan and expects no error.
- Astra 2 — fixed; test `kernel/tests/it/native_fields.rs:559` changes only the frame floor (34 then 44) and checks the second layout moves.
- Astra 3 — fixed; test `kernel/tests/it/content_region/native_fields.rs:208` exercises a cold pending field (count 1, then height 44 with the count unchanged).
- Astra 4 — fixed; test `host/linux/tests/it/native_fields.rs:141` scans pixels for dark ink on a white ancestor, and `host/linux/tests/it/native_fields.rs:163` checks the disabled blend against authored black.
- Astra 5 — fixed; test `kernel/tests/it/native_fields.rs:584` measures a height-200 rows-2 textarea as 30 (the auto twin) and checks nothing is published.
- Astra 6 — fixed; test `runner/tests/it/native_field_resize.rs:29` checks the no-padding content size is 100×30 and that feeding that size back does not grow the field.
- Astra 7 — fixed; test `host/apple/tests/it/content_region.rs:720` checks the `fieldContent` wire op, a same-size resize, and a null clear.
- Astra 8 — partly fixed; test `kernel/tests/it/native_fields.rs:622` checks the unclamped sibling baseline. Linux paint at `host/linux/src/paint.rs:1078` still clamps, and the test does not render.
- Grok 1 — fixed; `host/web/tests/focus.test.mjs:155` tabs a bare input and textarea (outline present, offset 2px); `NativeFieldsMacTests.swift:151` checks text and password masks; `host/linux/tests/it/native_fields.rs:132` checks the outer-box ring pixel.
- Grok 2 — fixed; the same web test compares disabled native color to a UA iframe and checks authored green wins.
- Grok 3 — fixed for the four rows; test `kernel/tests/it/native_fields.rs:650` covers both env paths and that an absolute ancestor font still inherits when there is no platform font. It does not cover `em` font-size or a later root-font change (finding 1).
- Grok 4 — fixed; `host/terminal/tests/cli.rs:110` checks black/white grid ink and the control font, and the Linux disabled test checks the blend.
- Grok 5 — fixed; `NativeFieldsIOSTests.swift:36` applies a synthetic provisional batch, expects count 1, and checks the frame is on screen.

**New findings:**

### 1. WEB_START is treated as a font boundary

**Severity:** material

**Where:** `kernel/src/txn/relative.rs:32` and `:59`. `kernel/src/arena/control_text.rs:91` now returns `WEB_START` for every native field when the host has set no control font.

**Failure:** On the web, `control_text_start` is `Some(WEB_START)`. That style’s mask is only text-align, text-indent, text-shadow, and text-transform; its `font_size` is the style default, 16, and font-size still inherits. The root-font loop updates a node only when `control_text_start(slot)` is `None`, and it does not walk descendants, so a runtime root font-size change skips an already laid-out native field that inherits the document font. Its cached text metrics stay at the old size. An authored `font-size` in `em` also takes `WEB_START.font_size` (16) as the parent: `font-size: 2em` inside a 32px parent computes as 32px, while the page (`font: inherit`) shows 64px. Padding and other non-font-size `em` still use the field’s computed font. The new reset test never sets `em` and never changes the root font after layout. Hosts that install `starting_style` (mask includes FontSize) keep the previous em-against-control-font behavior.

**Fix:** Use the starting style’s font size as the `em` parent only when its mask contains FontSize. Treat a native field as having no font boundary when `control_text_start` is `None` or its mask lacks FontSize, and include those fields in the root-font loop.

### 2. Linux paint still clamps the single-line centering offset

**Severity:** material

**Where:** `host/linux/src/paint.rs:1078`

**Failure:** The kernel’s single-line centering offset is now unclamped, and baseline-aligned siblings follow it. Linux still paints glyphs at `((content.3 - paragraph.height) / 2.0).max(0.0)`. With font 16, line-height 5 (used height 80), and content height 40, the offset is −20. Glyphs stay on the top of the content box while the siblings move up 20pt. Before this delta both sides clamped and matched. `oversized_single_line_uses_negative_centering_offset_for_baseline_alignment` checks the kernel baseline and does not render.

**Fix:** Drop the `.max(0.0)` so the painted origin uses the offset the kernel publishes.

### 3. Native macOS textarea styling allocates a text view

**Severity:** minor

**Where:** `host/apple/Sources/ExactKit/Mac/FieldChromeMac.swift:84`, called from `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:237`

**Failure:** `textareaInset` constructs a TextKit 2 `NSTextView` to read the default inset. `styleTextArea` calls it on every style application of a native textarea, not only on a chrome-cache miss.

**Fix:** Cache the inset in a static.