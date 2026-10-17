# Code review: Bold Text weights the system font (LLP 1115.000 D12.3, Tamber stage 6 A2), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~2..HEAD, the change and its first two rounds.
- **Method:** one brief (sha256 `ce8351529095060bd1534348f1e000afcbb6d2489b4f5c92d250c25adbfbcb59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (a P1: Android's default field and button title unstepped; P2s: Apple's Markdown classification, Android's light weights clamped).
- **Disposition:** applied without re-review (the last round), in the third-round commit.

---

I'll review the Bold Text weight changes read-only, starting with the review skill and the three commits from HEAD~2 through HEAD.The round-2 commit lists specific weight and invalidation fixes. I'll read those call sites and the host measure/paint paths next.No P0. Plain system text (a built-in generic stack) is one UIKit step at measure and at paint on Linux, Apple, and Android body text. Two defects are still open.

**[P1] Android's default field and button title never take the step — `kernel/src/arena/text_style.rs:134`**

`platform_control_weight` skips Bold Text whenever the weight's source is the host-registered control style. That is right for iOS, whose registered body face is already the bold `UIFont`. Android registers a plain 400 (`host/linux/src/paint/control.rs:44`). A default field and a default button title therefore stay 400, while a neighboring `text` goes 400 → 600. `Fields.face` then subtracts `fontWeightAdjustment` (`host/android/canvas/src/main/java/dev/exact/canvas/Fields.kt:797`), so the `EditText` adds that 300 back and still draws 400. The button face only sets the weight bit when the step changes the number (`kernel/src/arena/button.rs:212`); it does not, and `typography` writes 400 back (`host/linux/src/paint/button.rs:61`). The same field with an authored `font-weight: 400` does step to 600, because that source is the author's row. Layout and the `EditText` agree with each other; both miss the step.

**[P2] Apple steps a Markdown piece's own weight only when the catalog has no face — `host/apple/Sources/ExactKit/DisplayPreferences.swift:191`**

`boldTextSteps` is the live setting and `catalog[family]?.isEmpty != false`. `install_fonts` stores a generic descriptor for every stack id ≥ 8 (`host/apple/src/measure.rs:189`), including `font-family="system-ui, sans-serif"` and a runtime generic list. That catalog entry is non-empty, so `**bold**` stays 700. The kernel has already set `TextStyle::bold_text`, and Linux applies `bolder(700)` → 900 (`host/linux/src/text/markup.rs:51`). The measurer (`Text.swift:1365`) and the painter (`NodeText.swift:109`) share `boldTextSteps`, so the measured width matches the drawing; both are a step short of Linux. Built-in ids 0–7 are absent from the catalog, so a bare `serif` still steps and hides this.

**[P2] An Android field lighter than the device adjustment is clamped — `host/android/canvas/src/main/java/dev/exact/canvas/Fields.kt:799`**

Bold Text maps `font-weight: 100` to 200. With the platform's +300, `given` is −100, `coerceIn(1, 1000)` makes it 1, and the `EditText` draws 301. A `text` beside it paints 200. `font-weight: 200` lands at 301 instead of 300. `Fields.measure` and the live field both call `face`, so the box matches the glyphs.

What holds: the button mask is set when the step changes the title; SVG markers, masks, patterns, clips, and filters start from `computed_rows` and `embolden` once; `runtime_stacks` runs after the binding is interned and before the kernel reads it; a standalone iOS symbol is stepped once and passed as an explicit `SymbolConfiguration` weight; a Bold Text toggle dirties every node because `font-weight` is a layout row. The Linux chrome weights called out in the commit message are still queued there.
