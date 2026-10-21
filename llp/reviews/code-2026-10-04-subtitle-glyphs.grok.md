# Code review: the title view's subtitle as a line of symbols and text (371b61283), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `371b61283`.
- **Method:** one brief (sha256 `c0592c9446e432aa0f8cb1156f7c5cdc34145d8180ff3bfbaba84db9480bb295`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentence is omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** 1 taken (passive lines; no descent into controls). 2 taken: template symbols at the font's size with the run's font and dynamic colour, so dark mode follows; rebuilt on a content-size change; plain subtitles carry the same attributes. DEFERRED: negative fixtures for a button or badge after the heading.

---

1. **Should-fix — `line()` treats a button or a filled badge as the subtitle.** `host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:40`

`line()` accepts every non-paragraph whose shown children are only non-empty `symbolName` images and non-empty texts. It never looks at `isButton`, `actsAsButton`, a `press` handler, or a fill. `walk` then takes that node as the whole subtitle (`:55`) and does not look at a later text.

That matches a mute control nested in the pressable title row. `HeaderShape` still lists that control as a bar item, but the group is the `tap`, so `walk` still enters it. Direct children `image` + `text` match. A row wrapped inside the button matches one level down. A filled count or "Pro" pill matches too: `BadgeFace` is only considered before the heading (`:53`), so the same box after the heading becomes inline symbols and its background is dropped.

`when` and `display: none` are fine. Inactive `when` arms are destroyed, not hidden (`runner/src/instance/region.rs:266`), and `shown` drops `display: none` (`:36`). An empty `symbolName` (unknown role) fails the image test and rejects the box (`:44`). A `symbol:sf/…` name UIKit does not ship still qualifies; the draw loop then skips the image.

**Fix:** Make `line()` return nil for a button, a link or other `actsAsButton`, a node with a `press` handler, and a node with `channels("background_color") != nil`. In `walk`, do not descend into a button or pressable. Add a fixture case for each.

2. **Should-fix — The symbol is a tinted snapshot, so it stays the wrong size and color.** `host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:151`

```151:152:host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift
                guard let image = UIImage(systemName: name, withConfiguration: UIImage.SymbolConfiguration(font: font, scale: .small)) else { continue }
                out.append(NSAttributedString(attachment: NSTextAttachment(image: image.withTintColor(colour, renderingMode: .alwaysOriginal))))
```

`withTintColor(_:renderingMode: .alwaysOriginal)` resolves `.secondaryLabel` onto the image. `update` runs before the title view is in the bar (`:241`), and `HeaderTitleView` never rebuilds on trait changes. The text run keeps the dynamic color and follows the bar; the symbol keeps the color from the last `update`. In dark mode that is the light-mode secondary gray.

`.small` is smaller than the footnote the comment says the symbol matches. `adjustsFontForContentSizeCategory` (`:113`) can scale a preferred font inside the attributed string; it does not scale an attachment bitmap. Nothing observes `UITraitPreferredContentSizeCategory`, so after a Dynamic Type change the heading moves and the bell stays at the old point size. The attachment run also has no font or foreground color, so TextKit cannot restyle the symbol from those attributes.

The Latin fixture string is already in logical order, so this path does not reorder it. Baseline is whatever `NSTextAttachment` reads off that tinted image, and nothing tests it.

**Fix:** Build the attachment from an untinted `UIImage(systemName:)`, and put the footnote font and `.secondaryLabel` on that run. Keep the last `HeaderTitle` and rebuild on `UITraitUserInterfaceStyle` and `UITraitPreferredContentSizeCategory` with `UIFont.preferredFont(forTextStyle: .footnote)` resolved in the new traits. Drop `.small`, or scale the bounds from the font's cap height and still rebuild. Put the same font and color on the plain-subtitle attributed string.

Invalidation is sound. `source` includes each glyph's symbol and text (`:75`), `projectedSource` includes `group.source`, and `richTitle` runs on every sync. Renaming `bell` to `bell.slash` without changing "Muted  1w" still re-projects.

Accessibility matches the commit. The title view is one element; the value is the texts joined by two spaces (`:174`), and the hidden header's images are not walked. "Muted" and "1w" are the labels. An `aria-label` on a symbol is not copied into that value.

The fixture change is safe for web, macOS, Linux, and the agent. `muted` defaults to false, so the new `symbol:sf/` images are not in the tree until the chat control is pressed. Those hosts already paint `symbol:sf/…` as an empty box. The extra button sits on the chat route, which smoke does not snapshot.

The new test only checks the happy path: the value `"Muted  1w"` and an attachment count of 2 (`NavigationBasicsIOSTests.swift:116`). It does not name the symbols, change appearance or content size, or try a button, a badge, `display: none`, or a missing symbol. The `drawn` branch for a glyph line with no avatar and no press (`NavigationTitleIOS.swift:225`) never runs, because this chat title has both.

Verdict: LAND WITH FIXES
