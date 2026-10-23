# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), D1/D2 narrow re-check, D3 round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `01e51b5271`.
- **Method:** one brief (sha256 `d55d85bb8abf8a1c52d683a982ec2cd7ead207f7036752b6554b52162a568da4`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-check the D1 and D2 fixes and do a second-round D3 review of the write check, read-only, starting from the commits after the last combined review.The generic review workflow doesn't match this stage check. I'll read the fix commits and the write-check path directly, then judge them against the build's own refusals.The fix commits are in. Next I’ll compare the walk and the font/typed guards with lowering’s own refusals, including early exits in `prepare_element`.Auto-compacting conversation (80% full)...
Conversation compacted.
The previous pass already traced the fixes. I am confirming the line numbers in the walk and in `prepare_element` before writing the review.D1 and D2 hold. Nothing in those parts blocks. D3 still blocks, with two findings. No false refusal showed up: the new paths only drop rows, and a missed refusal is allowed.

The D1 fix is complete. An auto-weight face is stored as the path the app names, the canonical target, and that file's length and mtime (`contract/lower/src/fonts.rs:164`). If metadata fails at record time the length is `u64::MAX` (`fonts.rs:169`). `stale()` (`contract/lower/src/theme/rows.rs:119`) re-resolves the path and treats a different target, a missing file, or a different length or mtime as stale, which skips font rows rather than refusing them.

The D2 fix is complete. `typed()` (`rows.rs:312`) keeps a computed row only when every `Ident` in the value, including conditions and template holes, resolves to Slot, Derive, Resource, or Mutation. `folds()` marks a header's children only when the parent tag is `section` (`contract/lower/src/theme/scan.rs:277`).

`check_svg_with` (`contract/lower/src/svg.rs:766`) is the old `check_svg` body. `Lowerer::check_svg` (`svg.rs:520`) still runs its animation checks after the length check. `in_svg` parent tracking in the walk matches lowering, including `foreignObject`, `When`/`Each`/`Match`, and a nested `svg`. A `box_transform` error leaves the element.

1. **Blocker, D3.** `scan.rs:235` sets `left` only for an inherited left, a `box_transform` error, or `prepared` (unknown class, `check_svg_with`, any attribute whose name contains `animation` or `transition`). Rows are recorded at `scan.rs:249` whenever `left` is false, and descendants are walked from `scan.rs:278` with that flag. Every other `prepare_element` return before the style loop (`contract/lower/src/nodes.rs:496`) still keeps that element's style rows and its descendants. `node` (`contract/lower/src/lib.rs:933`) lowers children only after `prepare_element` returns `Ok`, so the build never judges those rows. A later write is refused with the style diagnostic; the build reports the earlier lowering diagnostic.

   Concrete cases, all with `color="ink"` (a longhand the walk keeps; `color` is shared, `svg.rs:295`):

   - `nodes.rs:47` — unknown tag `widget`. `check_svg_with` passes. The build reports `lower-unknown-tag`.
   - `nodes.rs:140` — `text` with `text-box` trim and `columns`. The build reports `lower-attr-tag` (`shorthands.rs:549`).
   - `nodes.rs:143` — an inline `text`/`time` run with `color-scheme` and another style row. The build reports `lower-attr-tag` on `color-scheme` and never judges the other row.
   - `nodes.rs:179` — `time` with no `datetime`. The build reports `lower-time-instant` (`live_time.rs:41`).
   - `nodes.rs:189` — `list listStyle="inset"`. `grouped::style` returns `lower-grouped-list` (`grouped.rs:88`). A grouped list whose `sections()` fails (`nodes.rs:195`), such as `list appearance="auto"` containing `column color="ink"`, never lowers that column (`grouped.rs:399`).
   - `nodes.rs:256` — `scroll` with no `height`, `max-height`, or `flex`, under a parent that stacks it. The build reports `lower-scroll-unbounded`.
   - `nodes.rs:478` — `font-family="Foo\\Bar"` beside `color="ink"`. `font_use` returns `lower-font-family-list` (`fonts.rs:487`) before any style row. `judge` (`rows.rs:652`) skips the font row and still judges the color row.

   The same hole is every other `?` and `return` before line 496: button appearance, painted radio group, field/menu/alert/exclusion checks, position area, control, nesting, progress, menu shapes, list validation, collection, media, `contain::positioned`, zero size, nav enter, `navigate`, state-on-a-run, press opacity, `fold_states`, `heading_style` (`nodes.rs:396`), scroll `document`, `control_word`, and a bad positional.

   **Fix:** before recording rows, run those pre-style guards (share them with `prepare_element`) and set `left` for the element and its descendants when any would return `Err`. Leave the style loop alone: `attr` errors are pushed and the loop continues (`nodes.rs:507`), so those rows stay judged. Setting `left` only drops rows.

2. **Blocker, D3.** After the style loop, three returns still skip children, and the walk keeps those children's rows (`scan.rs:278`). The parent's own rows were checked, so they should stay.

   - `nodes.rs:723` — `!can_hold_children()` (`kernel/src/node.rs:75`). An `svg` `rect`, `circle`, `path`, `use`, `image`, or `line` cannot hold children. A child `text`, `column`, or `button` with `color="ink"` is recorded: `in_svg` is false because that parent is not an SVG container (`svg.rs:77`), so `prepared` passes and the child looks like HTML. The build reports `lower-leaf-children` and never the color diagnostic. The same applies to `image`, `video`, and `hr`. A child `rect` of a `rect` is dropped only because it looks like an SVG element outside `svg`; that is not coverage of this case.
   - `nodes.rs:730` — a `text` paragraph's non-run child (`run` at `nodes.rs:733` allows `text`/`time` through `if`/`for`). `Text` can hold children, so the walk keeps `column color="ink"` inside a paragraph. The build reports `lower-leaf-children`.
   - `nodes.rs:639` — `appearance="textfield"` on anything other than a text field, after the style loop, on a parent that can hold children (`column appearance="textfield"` containing `text color="ink"`). The build reports `lower-appearance` and never lowers the child.

   **Fix:** keep the element's own rows. Push its descendants with `left` set when the node type cannot hold children, when a text leaf's child fails `run`, or when `textfield_elsewhere` (`fields.rs:116`) would return. Descendants of a pre-style failure are already covered by finding 1.
