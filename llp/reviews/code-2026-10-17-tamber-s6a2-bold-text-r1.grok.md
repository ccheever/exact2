# Code review: Bold Text weights the system font (LLP 1115.000 D12.3, Tamber stage 6 A2), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `82035845d066b7f5bcec097178d16f276d41996389ac9bf8ac63e899bdbece25`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (P1s: a toggle through `set_preferences` never committed with no reader, default text measured heavier than painted; P2s on SVG text, page symbols, native fields and buttons; a P3 on a failed commit).
- **Disposition:** in the first-round commit.

---

I'll review HEAD only, read-only, focused on Bold Text weighting the system font. First I'll read the review instructions and the commit itself.The review is HEAD only (D12.3), read-only. I'll inspect the commit and the bold-weight paths without touching the worktree.The commit is in. Next I’ll trace every weight read, the preference bit, and what a setting change actually invalidates.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll confirm the line numbers on the Bold Text paths before writing findings.No P0. Two P1s block the feature on the paths below.

**[P1] A Bold Text toggle through `set_preferences` never commits when the plan has no `exactViewport` reader — `runner/src/runner/viewport.rs:47`**

`set_bold_text` returns true when the flag changes, and the early commit also requires `viewport == self.viewport`. `bold_text` is inside `Preferences`, so a toggle makes the viewports differ and that commit is skipped. `replace_viewport` stores the new viewport, then `recommit` (`runner/src/runner/admission.rs:19`) returns `Ok(None)` when nothing reads `exactViewport`, and `bold_text_next` stays pending. Turning Bold Text on (the iOS notification → `tellPreferences`, or Android `fontWeightAdjustment` → `set_device`) on such an app leaves every run at the old weight until some later commit. Boot (`runner/src/runner.rs:827`) and `Runner::set_bold_text` (`runner/src/runner/root_font.rs:108`, which the contract test calls) do commit. A plan that reads `exactViewport` commits through `recommit`.

**[P1] Default system-font text is measured one step heavier than it is painted — `kernel/src/arena/defaults.rs:212`**

`computed_style` writes the stepped weight and leaves `FontWeight` clear on the mask. `style_json_rows` (`host/apple/src/style.rs:132`) emits a value only for a set mask, so an unmarked initial 400 is absent from the style dictionary. Paragraph paint (`NodeText.swift:15`, `InlineText.swift:67`) and the bar face (`BarFaceIOS.swift:92`) then use 400, and `cssWeight` (`Text.swift:758`) pins `wght` to 400, which cancels UIKit's own Bold Text. Measurement uses `text_runs` (`kernel/src/arena.rs:679`), which steps 400→600 with no mask check. A `text "Hello"` with no authored weight, no text-style start, and no document weight lays out at semibold width and draws regular. An authored or text-style weight is masked, stepped, and sent, so those agree. The bar face's own +200 is gone, so an omitted row stays regular. `BarFaceIOSTests` only shows that the face adds no step of its own.

**[P2] SVG text steps the wrong nodes — `kernel/src/svg/scene.rs:319`**

The `svg` root starts from `computed_style` (`scene.rs:285`), already stepped. `cascade` clones the child's raw style and copies inherited rows the child does not set, with no per-node `emboldened`. A `text` or `tspan` that sets `font-weight` keeps the CSS weight (`scene/text.rs:180` reads it straight). A `tspan` that names a shipped family and inherits the weight receives the parent's already-stepped number. On a `system-ui` svg, `<text font-weight="400">` stays regular while the paragraph beside it is semibold, and `<tspan font-family="Georgia">` draws at the parent's stepped weight. Host measure and paint both follow that scene style.

**[P2] Page symbols take UIKit's Bold Text on top of the kernel's step — `host/apple/Sources/ExactKit/SymbolMeasure.swift:31`**

Measure asks `UIImage` for the weight `computed_style` already stepped (`kernel/src/kernel/intrinsic.rs:104`). Paint does the same when the row is in the style dictionary (`NodeViewIOS.swift:543`). A configured symbol weight is the base; a trait collection with `legibilityWeight == .bold` resolves one step heavier. A hand-built symbol at `.regular` with Bold Text is semibold. Exact's star at CSS 400 is configured semibold and draws bold, and the measured box follows. Bar-face symbols pass point size only (`BarFaceIOS.swift:99`), so they take only UIKit's step. When the row is unmarked, paint falls back to 400 plus UIKit's one step, while measure still sends the stepped weight, so the box is heavier than the glyph. Linux `glyph_face` and Android `setWeight` use the kernel weight once.

**[P2] Native fields and authored-weight native buttons stay at the CSS weight — `kernel/src/arena/text_style.rs:112`**

`emboldened` skips a native button, a native field, and a direct child, on the premise that the platform weights them. The field then pins `wght` to that unstepped number (`NativeFieldsIOS.swift:104`). A native button whose style dictionary contains a size or a weight does the same (`ButtonConfigurationIOS.swift:17`). A hand-built `UITextField`, and a `UIButton` title at `.regular`, become semibold under Bold Text. Exact's field and its `font-weight: 400` button stay regular. A button with neither size nor weight returns UIKit's incoming font, which is the path the simulator note describes.

**[P3] A failed preference commit leaves Bold Text pending — `runner/src/runner/viewport.rs:84`**

`set_bold_text` has already stored `bold_text_next`. `replace_viewport` rolls the viewport back on error and leaves the pending flag. The next successful commit weights the system font after the preference change was refused. The early path (`viewport.rs:50`) does roll the flag back. A real toggle takes the other path.

Checked and clean: the kernel applies one step (`computed_style`, `text_style`, paragraph runs); `bolder`'s stops match the UIKit table in the RFC; Linux and Android painters do not add `fontWeightAdjustment` again. Web/wasm leave bit 14 unset on purpose (`host/web/src/abi.rs:121` unpacks bits 0–7 and 26–31; the RFC says the web has no Bold Text setting), and that unpack does not land on runner bit 14. Apple (`DisplayPreferences.swift:127`), Linux (`device.rs:79`), and Android (`Device.kt` `boldText`) all set the runner bit. Boot applies the flag before the first commit.

The contract test calls `Runner::set_bold_text`, so it never hits `set_preferences`. The bits round-trip loops `0..32` (`runner/src/viewport.rs:605`), which stops before bit 14.
