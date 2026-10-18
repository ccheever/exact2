# Code review: Bold Text weights the system font (LLP 1115.000 D12.3, Tamber stage 6 A2), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the change and its first round.
- **Method:** one brief (sha256 `f1d0a9cfa0ec69d8bd426ab88e9c94b01ca134a1af727e6bf0849cd1aec8e07c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (P1s: marker, mask and pattern text stepped twice, Linux and Android button titles losing the step, iOS standalone symbols unstepped).
- **Disposition:** in the second-round commit.

---

I'll review the Bold Text weight change read-only: round-1 fixes in HEAD against HEAD~1, then measurement versus paint and invalidation on each host.Both commits are in scope. I'll read the round-1 fixes and the paths that measure or paint weight.The round-1 diff is in view. Next I’ll trace weight from measurement through each host’s paint path and the invalidation commit.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll confirm the round-1 fixes and the remaining step mismatches against the current diff, then report only defects I can point at in the code.Nothing is P0. Three weight bugs are still open.

[P1] Marker, mask, and pattern text steps twice — kernel/src/svg/scene/text.rs:177

Round 1 starts the svg root from `computed_rows` and steps each run once. A marker, mask, and pattern still start from `computed_style` (`kernel/src/svg/scene/marker.rs:304`, `kernel/src/svg/scene/island.rs:107`, `kernel/src/svg/scene/island.rs:222`), which has already stepped a system weight and copied it into the inherited rows. `cascade` gives that weight to a `<text>` that does not set `font-weight`, and `runs()` steps it again. With Bold Text on, a marker `<text>` at system 400 is anchored and painted at 800 on Apple and on Linux (`svg_text` draws the scene run); the same text in the svg root is 600. A named family in that marker that does not set `font-weight` keeps the baked 600, so Inter paints semibold. A text node that sets its own `font-weight` steps once.

[P1] Linux and Android drop the step on an unauthored button title — kernel/src/arena/defaults.rs:217

`computed_style` steps a non-iOS button title and sets the `FontWeight` bit so the host paints the measured weight. `button_face_style_unresolved` then replaces the mask with the button’s and title’s authored bits (`kernel/src/arena/button.rs:202`), which clears that bit when neither wrote `font-weight`. `typography` and `neutral_typography` (`host/linux/src/paint/button.rs:61`, `:31`) replace an unmasked weight with 400 or the look’s control weight. `measure()` and paint share `face_layout`, so both use that unstepped weight. With Bold Text on, `button "Save"` with no `font-weight` paints at 400 (neutral: the control weight) while the same words in a `text` node paint at 600. A title that writes `font-weight`, or names a text style, keeps the bit and steps once. An iOS button whose title is in UIKit’s own font is left to UIKit, which bolds it once.

[P1] iOS standalone symbols stay at the unstepped weight — kernel/src/arena/text_style.rs:146

`emboldened` skips every iOS image so the symbol configuration can take the Bold Text trait. `NodeViewIOS.updateSymbol` always builds `SymbolMeasure.configuration(points:weight:)` and defaults a missing `font_weight` to 400, an explicit `SymbolWeight`. The image is cached under that weight, and the key does not include the trait, so a Bold Text change that leaves the style JSON the same does not rebuild it. Bar-face symbols, which do follow the trait, use `SymbolConfiguration(pointSize:)` with no weight (`BarFaceIOS.swift:146`). With Bold Text on, `image "symbol:sf/star"` measures and draws at regular while body text is semibold. An authored symbol weight is skipped too, because the exclusion is the node type. An unfonted button symbol follows the title font, which UIKit bolds.

Body text, paragraph struts, fields, authored button titles, and bar titles take one step, and measurement matches paint. A generic-first stack steps, a named-first stack does not, and a Bold Text change commits with no resource reader and rolls back when that commit is refused. Invalidation touches every live node, remeasures text and symbols, and Apple rebuilds SVG scenes from that touch list. Web and Mac never set the bit. Tests cover `text_style`, the mask bit, a generic stack, and the preference commit. They do not cover buttons, symbols, or SVG markers. A Linux file-field button word and an alert title assign a weight after `computed_style`, and material clock and menu glyphs read `node.style`, so those chrome weights stay where that assignment left them.
