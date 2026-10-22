# Code review: iOS grouped lists: a custom row's leading symbol and line as a standard cell's (Harmony), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e7671be379`.
- **Method:** one brief (sha256 `575ebc306a3c59d492d790cc9ad29942a40ed32732b1678d651b7fdf703e7510`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 72465c42f6

**VERDICT: WRONG**

The on-screen Harmony case is in place: a visible custom row's sheet slot (contain, 24×20) is marked, the large-scale glyph is centered and unclipped, an authored fit or size is left alone, and a visible row's line follows an inset change in the same turn. Three correctness holes from round 1 are still open.

1. **should-fix** — A custom row that is not on screen keeps UIKit's 16 pt separator, including after it scrolls into view. `GroupedListIOS.swift:608`, `GroupedCustomRowsIOS.swift:32`, `GroupedListIOS.swift:780`

The handler no longer reads the row. It uses `customLooks[id]?.line`, and that line is written only in `look`, which runs from `carry`. `mount` carries `visibleCells` only, then consumes `resized`. The layout pass at the end of `mount` therefore bakes the automatic margin for every custom row that has not been carried. Scrolling one in configures the cell, `look` stores 56 and sets `resized`, and nothing invalidates until the next batch's `mount`. Round 1's live read in the handler had this right for every row; storing the line at carry time dropped it. The new test only moves the inset on the row that is already visible.

2. **should-fix** — In right-to-left, a symbol row's line is inset by the end padding (16), not by the content start (56). `GroupedCustomRowsIOS.swift:64`

`separatorLeading` uses `row.bounds.width - box.maxX` when the collection is right-to-left. The sheet's padding is physical (`padding-left` 56, `padding-right` 16), and `Kernel::grouped_separator` starts at that physical left. For a 370-wide row the formula returns 16. UIKit then applies `bottomSeparatorInsets.leading` from the right, so the line occupies the leading end and runs under the symbol. A standard cell beside it insets from the leading edge by the symbol margin. A symmetric 16/16 row happens to match; a symbol row does not. `x` is the row's physical `minX` and is added on that from-the-right measurement. There is no right-to-left test.

3. **should-fix** — `listSymbol` and `CustomLook` outlive a row that leaves the list. `GroupedListIOS.swift:666`, `GroupedCustomRowsIOS.swift:12`, `GroupedListIOS.swift:1044`

The flag is cleared when `look` swaps the marked view, when that same id is configured as a standard cell (`forgetLook` at `GroupedListIOS.swift:1044`), and when the list is removed (`GroupedListIOS.swift:1297`). `update` replaces `rows` and never forgets ids that dropped out of the model. `CustomLook.symbol` is a strong `NodeView`, so a destroyed image stays alive for the life of the list after `removeFromSuperview`. A children op that reparents that same view (the view is not destroyed) shows it outside the list still drawn as a list symbol: large scale, unclipped, `contentMode` center. `restore` puts carried rows home without clearing the flag; the next `carry` fixes a row that is still visible and still custom, and does not run for one that left. `recycle` / `rebind` / `finishReuse` also leave the flag, which matters if a marked image is ever parked.

What holds up:

- Setting `listSymbol` inside `carry` does not loop. The setter returns when the flag is unchanged (`NodeExtrasIOS.swift:331`), `updateSymbol` only rebuilds when the key changes, and `queueIntrinsicSize` flushes on the next turn with `SymbolMeasure.size`, the same medium-scale size the kernel already measured. `resized` is cleared in `mount` after one invalidate, and the next `look` sees the same line.
- The queued intrinsic size ignores the large scale. The sheet's width and height are 24 and 20, so the box does not follow that report. The drawn `UIImage` is the large glyph; hit testing uses the 24×20 view bounds. `finishReuse` still reports `image?.size` (the large image). Grouped rows are not parked, so that path does not run for them.
- Last row of a section is left on UIKit's configuration (`path.item + 1 < rows.count`). An authored bottom edge is still hidden via `owned`. A custom row with no leading symbol stores the content-box start, which is the 16 pt row inset. Setting only `.leading` on the configuration the handler was given keeps UIKit's trailing inset; the passing test shows the line at 56 and matching the standard row.
- The glyph is the large scale of the node's font size and weight. Image nodes receive inherited font rows, the row's size is `-exact-body`, and that ramp is `UIFont.preferredFont` for `.body`, with weight 400. An authored `font-size`, `font-weight`, or `symbol_rendering` still feeds `symbolConfiguration`; the large scale is applied on top of that base. An authored `object-fit` other than `contain`, or a box other than 24×20, skips the mark. The slot itself stays 24×20 at every text size (the metric is not a ramp), so at the largest accessibility sizes the unclipped glyph outgrows the slot while UIKit's own cell grows its image.
- tvOS compiles the mark and not the separator handler (`itemSeparatorHandler` is already inside `#if !os(tvOS)`). Mac Catalyst takes the iOS path. `NodeViewIOS.swift` is 1482 lines and was not touched in round 1. `GroupedListIOS.swift` is 1496, under the 1500 cap. `contentBox()` as `package`, and `resized` / `customLooks` as module-visible, are what the other file in `ExactGroupedLists` needs. The `symbolLookKey` comment sits on `symbolLookKey` again.
