# Code review: the title view's subtitle as a line of symbols and text, round 2 (ace88318d (on f8b2de4da)), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `f8b2de4da`.
- **Method:** one brief (sha256 `7a6389604005400321016e033796e1a7b51bc3a551c61b6948c43eb9c74a30d9`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken (the same control set as `passive`, with the heading exception). 2 taken: the test requires the exact return state (no value, hidden).

---

**Round 1**

1. **Not fully resolved.** A glyph line and its direct children must be passive (`NavigationTitleIOS.swift:44`), so a `button`, a `press`, or a fill on that box is rejected. The walk still only skips `isButton` and `press` (`:77`). A `row`/`column` with `role="link"` or `role="button"` is `actsAsButton` and is not a `button` kind (`host.rs` maps only `Pressable` to `"button"`). It is entered, and the plain-text arm (`:67`) then takes its text as the subtitle.
2. **Resolved.** Symbols are template images at the footnote font, with the run’s font and `.secondaryLabel`. Plain subtitles get the same attributes. Size is rebuilt on a content-size change.
3. **Resolved as far as it was taken.** Paragraph direction is `.natural` (`:172`). Per-run bidi isolation stays deferred, which matches the disposition.
4. **Resolved.** The line’s `aria-label` is `spoken`, otherwise the joined texts (`:72`, `:89`). The fixture sets that label.
5. **Resolved in part, deferral acceptable.** Attachments are checked as template images and the run’s color is the label’s `textColor`. A glyph-only title and negative recognition fixtures stay deferred. The return trip still accepts a hidden subtitle (`NavigationBasicsIOSTests.swift:129`); that was not in the deferral.

**Template symbols and `.foregroundColor`.** Yes, on iOS 17–26, for a symbol image. `NSTextAttachment(image:)` is documented to follow the surrounding font and color, and a monochrome SF Symbol picks up the label color (WWDC21 session 10251). `UIImage(systemName:)` is already `.alwaysTemplate`; `withRenderingMode(.alwaysTemplate)` does not change that. `.secondaryLabel` stays a dynamic color on the run and on `subtitle.textColor`, so dark mode resolves at draw time and does not need its own rebuild. A raster template does not do this. The reliable path is the one here: an untinted symbol in `NSTextAttachment(image:)`, with `.font` and `.foregroundColor` on that run. `withTintColor(_:renderingMode: .alwaysOriginal)` bakes the tint.

**Content-size registration.** No retain cycle: the closure uses the `view` argument UIKit passes in and does not capture `self` (`:153`), which is the documented pattern. No reentrancy: `update` does not change `preferredContentSizeCategory`, and a nested call would only rewrite the same `shown` title. No tvOS compile break: the file is `os(iOS) || os(tvOS)`, deployment is 17, and `registerForTraitChanges` / `UITraitPreferredContentSizeCategory` are tvOS 17, same as `ControlsIOS.swift:103`.

**Skipping a control when the heading is inside it.** The exception is right. `!heading.isDescendant(of: child)` (`:77`) is what still enters a `button` that wraps the heading. HeaderShape does walk a `button` that has no `press` handler, so skipping every control would drop that subtitle. A nested `press` does not contain the heading: HeaderShape never records a heading behind one, and those stay skipped.

**`spoken`.** It is invalidated. It is part of `source` (`:94`), `projectedSource` includes `group.source` (`NavigationBarIOS.swift:373`), and `richTitle` calls `update` on every prepare (`:277`), which sets `accessibilityValue` from `spoken` (`:209`).

1. **Should-fix — A link or `role="button"` view inside the title group is still read as the subtitle.** `NavigationTitleIOS.swift:77` (taken at `:67`). `passive()` treats `actsAsButton` as not a line, but the walk does not. For a title row whose heading is followed by `row role="link"` (or `role="button"`) containing text, or a symbol plus text, that row is not `isButton` and has no `press`, so it is entered and its text becomes `subtitle`, ahead of a later real subtitle. **Fix:** skip the same set `passive()` uses, and keep the heading exception:

```swift
if child.isButton || child.actsAsButton || child.handlers.contains("press"), !heading.isDescendant(of: child) { continue }
```

2. **Nit — Turning mute off still passes if the subtitle is merely hidden.** `NavigationBasicsIOSTests.swift:129`. `accessibilityValue == "Online" || view.subtitle.isHidden` passes when the value stays on the aria-label and the label is hidden. **Fix:** require `view.accessibilityValue == "Online"` and `!view.subtitle.isHidden`.

Verdict: LAND WITH FIXES
