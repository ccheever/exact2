# Code review: a grouped-list section without its card (50cf88e90), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `50cf88e90`.
- **Method:** one brief (sha256 `09173566aca7581698b841497c4dcee4d18c147484a7140f703d132e75ca6b30`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** 1 taken (class refused). 2 taken by narrowing to the literal. 3 taken (highlight handler). 4 taken (sheet separators and grouped borders dropped).

---

**1. Should-fix — A class `background-color` comes back onto the section after the literal is moved, so the web keeps a card.** `contract/lower/src/grouped.rs:378`, `contract/lower/src/lib.rs:697`

`section()` strips `background-color` before class expansion. `class_rows` then treats the property as unset and copies the style’s value onto the section. With `style Card` / `background-color #fff` and `section class="Card" background-color="transparent"`, the group is transparent and the section is `#fff`. On the web that `#fff` shows through the group, so the card does not drop and the literal loses to the class. iOS reads only the group, so `card` is false and the cells clear. Same split for a transparent class and no literal: the opaque `CELL` group stays, which §6.2 notes, but it contradicts D7 (a class replaces the sheet the same way an attribute does). LLP 1091 does not add a further case: components are inlined before lowering, so a component’s own literal still moves; an imported `style` is still just a class.

Fix: expand classes first, then move the winning `background-color` (the element’s own value over the class, including a `class=(cond ? A : B)` ternary) onto the group only. Do not refuse the attribute.

**2. Should-fix — iOS implements a single boolean, so a coloured card and a one-sided `light-dark()` stay on the system fill.** `kernel/src/grouped.rs:135`, `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:362`

`card` is false only when both appearances have alpha 0. `None` (what `currentColor` stores) counts as a card. Any other colour, including `light-dark(transparent, #fff)` and `#ffeeee`, sets `card` true, and the cell then uses `defaultBackgroundConfiguration()` — the system grouped fill, not the group’s colour. The sheet paints the author’s colour, and a one-sided pair really does drop the fill in that appearance. Appearance changes do not rebuild the bool, because it already ORs both sides. A runtime change of the group’s colour does reach `grouped_list`: the style op walks up to the list through `touchedAndAbove`. `is_none_or` is fine here; the toolchain is 1.97 and the kernel already uses it.

Fix: send the group colour (both sides) on the section. On iOS, use that colour as the resting background, dynamic for `light-dark()`, and set `showsSeparators` from the current appearance’s alpha, invalidating on trait change. Reconfigure when the resolved colour changes, not only when the bool flips.

**3. Should-fix — `.clear()` removes the tap highlight from a standard row in a card-less section.** `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:362`

`UIBackgroundConfiguration.clear()` has no per-state defaults, so `updated(for:)` stays clear. `shouldHighlightItemAt` is still true for a pressable, enabled, non-inert button (`:538`), and D5 says that row highlights. Custom rows should not: their `NodeView` takes the touch (§4). Disabled and non-button rows already return false from `shouldHighlight`. `showsSeparators` is the right per-section API (`NSCollectionLayoutSection.list(using:)` at `:260`); UIKit does not ignore it. `recarded` (`:290`) reconfigures existing rows and invalidates layout (`:309`) when that section’s bool flips; first appearance configures cells after `model` is updated, so it does not need the same path.

Fix: keep `defaultBackgroundConfiguration()` and, in `updateConfiguration(using:)`, set `backgroundColor = .clear` only when the section has no card and the cell is not highlighted or selected. Leave custom rows unhighlighted.

**4. Should-fix — The sheet still draws separators on a card-less section.** `contract/lower/src/grouped.rs:415`, `contract/lower/src/grouped.rs:600`

macOS, Linux, and the web have no grouped-list host; they paint this sheet. Moving `background-color` onto the group does make a transparent or coloured card there. Row `border-bottom` and, for `listStyle="grouped"`, the group’s top and bottom borders stay. iOS turns separators off. A single inset row hides its one border by the group’s `overflow: hidden`, which is why a one-row profile header matches; a second row, or a grouped-style edge, does not.

Fix: when the winning literal is transparent on both sides, omit those borders in `row()` and on the grouped group. For a one-sided `light-dark()`, make the border colour transparent on the transparent side.

A `when` is not an attribute. A ternary or `match` value is moved with the attribute and resolved at runtime; a `when` that swaps whole sections is already walked by `over()`. No separate change for that.

Tests miss every case above. `a_transparent_section_has_no_card` is a bare literal, and the section check allows any dark colour (`grouped_list.rs:449`). `testACardlessSectionsCellsAreClear` assigns `card` on the Swift model, so it never decodes `"card"` and never checks a standard row, the highlight, or separators.

Verdict: LAND WITH FIXES
