# Code review r2: a grouped-list section without its card, 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at the round-1 artifacts commit.
- **Method:** one brief (sha256 `80522b31bdfe2df64b8cf795f859d0ad62b71fb21a64fd3ec6b576beef6b4ee0`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** DO NOT LAND.
- **Disposition (r3):** 1 taken: a card-less section's list configuration gets a clear background. DEFERRED: a test reading the section configuration (not exposed). 2 taken: no border-radius on an inset card-less group.

---

**Round 1**

- Astra 1 (tvOS `showsSeparators`) is resolved. The assignment is inside `#if !os(tvOS)` at `GroupedListIOS.swift:260`. Separators staying on tvOS is the platform limit the RFC already states.
- Astra 2 / Grok 2 (a colour or a one-sided `light-dark()` disagreeing across hosts) is resolved by narrowing. `section()` admits only the literal `"transparent"` and refuses anything else (`grouped.rs:381`).
- Astra 3 / Grok 4 (sheet separators and grouped edge borders) is resolved. Card-less rows get `border-bottom-width: 0` and `margin-bottom: 0` (`grouped.rs:623`), and a `grouped` section skips its outer borders (`grouped.rs:438`).
- Astra 4 / Grok 3 (`.clear()` eating the press highlight) is resolved for a pressable standard row. The configuration update handler restores the default background while highlighted or selected (`GroupedListIOS.swift:369`).
- Astra 5 / Grok 1 (a class background winning after the literal is stripped) is resolved by refusal. A `class` beside the literal is an error (`grouped.rs:393`). A class with no literal does not drop the card; the group stays the system fill, which §6.2 now states.
- Astra 6 (the UIKit test never reconfigured a standard row) is resolved. `testACardlessSectionsCellsAreClear` drives row 20 through both card transitions and invokes the highlight handler. Separator assertions on iOS stay deferred, as the round-2 note says: the layout configuration is not readable from the test.

`over` walks `when`, `each`, and `match`, so a section under `when` is checked the same way as a direct child. LLP 1091 inlines components before lowering, so a module section whose attribute is the literal `"transparent"` (including a prop substituted to that literal) drops the card. A non-literal, or a `class` beside the literal, is refused. A cell dequeued for another row, or reconfigured when its section’s `card` flag flips, goes through `configure`, which installs or clears the handler. A row’s view id does not move to another section. A custom row in a card-less section keeps `highlights` false, which matches the RFC: no cell highlight. `didSelectItemAt` still deselects immediately, and the handler treats that brief `isSelected` like a press.

**Findings**

1. **Blocker — The card-less section still paints UIKit’s section fill.** `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:261`

   `showsSeparators` is turned off and each cell’s background configuration is cleared, but `UICollectionLayoutListConfiguration.backgroundColor` is left nil. Nil means the appearance’s system section color, and for `insetGrouped` and `grouped` that color is the continuous rounded fill behind the cells. Clear cells show that fill, so a profile header still sits on a card. `testACardlessSectionsCellsAreClear` only reads `cell.backgroundConfiguration`, so it passes while the decoration is still there.

   **Fix:** in `layout()`, when `s?.card == false`, set `c.backgroundColor = .clear` (available on tvOS as well as iOS). Keep the cell background clear. Extend the test to read the section’s list configuration background after layout, for a card-less section and for the section that keeps its card.

2. **Should-fix — An inset card-less group is still clipped to the card’s corner radius.** `contract/lower/src/grouped.rs:451`

   `inset` still adds `border-radius: 26` and the group still sets `overflow: hidden` when the background is transparent. The sheet clips a header image to the rounded card. With the section fill cleared, iOS does not clip the row’s contents to that radius.

   **Fix:** when `cardless`, skip `border-radius`. Keep the 16-pt side margins so the section stays inset. `overflow: hidden` can stay; without a radius it only clips to the rectangular group, which is what hid the last separator.

Verdict: DO NOT LAND
