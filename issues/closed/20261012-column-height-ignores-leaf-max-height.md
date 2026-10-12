# A flex column sized in definite space takes a leaf's whole content height past the leaf's max-height (a field-sizing="content" textarea, a text)

**Status:** Closed
**Resolution:** fixed by #496
**Systems:** kernel layout, vendored Taffy flexbox
**Severity:** P2
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-12
**Related:** T3 PR #479 (diff-cite-popover); https://github.com/ccheever/exact2/blob/feat(example)/t3-code-diff-cite-popover/examples/t3-code/EXACT2-GAPS.md (the clone's X86, recorded there as "X84" before the number was taken); `vendor/taffy/EXACT-PATCHES.md` (patch 14 changed the same function's max-content branch)

## Summary

A `column` (a flex container, `flex-direction: column`) with an automatic height should be as tall as its
items' clamped heights. CSS gives the same answer whatever the available height is. In Exact the column's height
comes out wrong in one case: the available height is definite and an item is a leaf (a `text`, a `textarea`) with
a `max-height`. The column is then as tall as the leaf's whole, unclamped content, while the leaf itself is laid
out clamped. The space below the leaf is empty, and anything that follows the column moves down. A popover's
`column` holding a `textarea field-sizing="content" max-height="10rem"` grows with its text past that maximum
(case D below), while the textarea stays at its maximum and scrolls.

Four cases are fine: a box (not a leaf) with the same `max-height` in the same column (G below), a `row` (F), a
block (`view`) parent (E), and a column inside a block of definite height (H), because a block offers its children a
max-content height (`vendor/taffy/src/compute/block.rs:1034`). A column is in definite space when it is a row's item
that is not stretched (A to C), or absolutely positioned (D, standing in for the clone's popover).

The likely site is Taffy's `determine_container_main_size`
(`vendor/taffy/src/compute/flexbox.rs:1449`). When the main axis's available space is `Definite`, an item's
length is

```rust
// vendor/taffy/src/compute/flexbox.rs:1462-1467
let item_main_length = |child: &FlexItem| {
    let padding_border_sum = (child.padding + child.border).main_axis_sum(constants.dir);
    (child.flex_basis.maybe_max(child.min_size.main(constants.dir))
        + child.margin.main_axis_sum(constants.dir))
    .max(padding_border_sum)
};
```

That is the flex base size floored by `min-height`, never capped by `max-height`. The `MinContent | MaxContent`
branch of the same function caps each contribution at the item's `max_size` (`maybe_clamp(style_min, style_max)`,
lines 1598-1600 and 1675-1677). The `MinContent if is_wrap` branch (line 1520) uses the same floor-only formula as
the `Definite` branch.

Only a leaf shows the difference. Its flex base size is its content size, measured under `SizingMode::ContentSize`,
and `compute_leaf_layout` drops min and max sizes in that mode (`vendor/taffy/src/compute/leaf.rs:61-62`:
`SizingMode::ContentSize => (known_dimensions, Size::NONE, Size::NONE)`). A block or flex container clamps its own
result in either mode (`block.rs:662`), so a box's base size is already clamped. The hypothetical main size, which is
the base size clamped (`flexbox.rs:1250-1251`), is right, and that is the size the item is laid out at.

Taffy alone reproduces it, without the Exact kernel. In a probe against `vendor/taffy`, a 200-wide flex column sits
in a row with `align-items: flex-start`. Its one leaf child measures 656 tall and has `max-height: 130`. Under a
definite available height of 900 the column is 656 and the child 130. Under a max-content offer both are 130. A block
container, or a box child, is 130 either way. The record has the source and the output. Upstream Taffy's main
(`7d33901cc`, 2026-10-09) has the same `Definite` branch.

## Why this arose

T3 Code's citation comment is a `textarea` with Tailwind's `field-sizing-content max-h-40` in a popover. The clone
(T3 PR #479, task diff-cite-popover) wrote the same thing in a `column`. Live on macOS, an 8,001-character comment
made the popover about 16,000 pt tall and blank, while the textarea stayed at its 10rem maximum. The clone's probe
measured a 320-wide column at 16,018 pt for a 2,500-character unbroken value and 8,738 pt for 1,220 characters of
prose, against a textarea 130 pt tall. The clone works around it with a box held at 4rem to 10rem
(`overflow="hidden"`) around the textarea. A box's max-height is honoured (case G below), so the popover stays near
180 pt and the text scrolls. The clone declares this as X86 in its `EXACT2-GAPS.md`.

## Reproduction

Main `dcd101a14fbc2c225411f1aa827d35a17d1abd3f`, macOS 26.6.2 (25G83), Apple Silicon. The web target ran in headless
Google Chrome 155.0.8059.39 on the same Mac. This is a one-file app made with `bun scripts/exact.mjs new <dir>`. Its
`app.ts` answers `texts()` with `{ unbroken: 'x'.repeat(2500), prose: 'The quick brown fox jumps over the lazy dog.
'.repeat(28).slice(0, 1220).trimEnd() }`.

```text
shape Texts
  unbroken: string
  prose: string

component Probe
  resource t = texts() as shape Texts
  view
    main testId="root" width="100%" height="100%" display="flex" flex-wrap="wrap" align-items="flex-start" align-content="flex-start" gap=8 padding=8 box-sizing="border-box"
      column testId="col-a" width=320
        textarea testId="ta-a" value=t.unbroken field-sizing="content" min-height="4rem" max-height="10rem"
      column testId="col-b" width=320
        textarea testId="ta-b" value=t.prose field-sizing="content" min-height="4rem" max-height="10rem"
      column testId="col-c" width=200
        text t.prose testId="tx-c" max-height="10rem" overflow="hidden"
      view testId="block-d" width=200 height=300 position="relative"
        column testId="col-d" position="absolute" top=0 left=0 width=200
          textarea testId="ta-d" value=t.prose field-sizing="content" max-height="10rem"
      view testId="block-h" width=200 height=300
        column testId="col-h"
          textarea testId="ta-h" value=t.prose field-sizing="content" max-height="10rem"
      view testId="block-e" width=200
        textarea testId="ta-e" value=t.prose field-sizing="content" max-height="10rem"
      row testId="row-f" width=200
        text t.prose testId="tx-f" max-height="10rem" overflow="hidden"
      column testId="col-g" width=200
        column testId="box-g" max-height="10rem" overflow="hidden"
          text t.prose
```

Run `bun exact.mjs agent <macos|web> --size 1400x900 "clock data" layout`. Heights are in points on macOS and CSS px
on the web. `rem` is the platform's: 13 on macOS, 16 on the web, so 10rem is 130 and 160. A textarea's border box adds
its border, and on the web its padding.

| Case | macOS: container | macOS: leaf | Chrome (web target): container | Chrome: leaf |
|---|---|---|---|---|
| A: `column` (a row's item, `align-items: flex-start`), textarea, 2,500 unbroken characters | **898** | 132 | 178 | 178 |
| B: the same, 1,220 characters of prose | **434** | 132 | 178 | 178 |
| C: the same `column`, a `text` leaf with `max-height` | **656** | 130 | 160 | 160 |
| D: an absolutely positioned `column` (like a popover's box), textarea | **706** | 132 | 178 | 178 |
| E: a block `view`, textarea | 132 | 132 | 178 | 178 |
| F: a `row`, a `text` leaf (the cross axis) | 130 | 130 | 160 | 160 |
| G: a `column` holding a box (not a leaf) with `max-height` | 130 | 130 | 160 | 160 |
| H: a `column` in a 300-tall block, textarea (measured under a max-content offer) | 132 | 132 | 178 | 178 |

The oracle is Chrome on plain HTML, with no Exact CSS: a `display: flex; flex-direction: column` div in the same row,
holding a `textarea { field-sizing: content; min-height: 4rem; max-height: 10rem }` or a div with `max-height: 10rem`.
Chrome gives column 166 and textarea 166 for both values (A and B), and column 160 and div 160 for C. In every case
Chrome's column is the clamped item's height.

Record (the contract, `app.ts`, both drives' output, the plain HTML page with Chrome's measurements, and the Taffy-only
probes, a column's and a row's, with their output):
https://raw.githubusercontent.com/ccheever/exact2/39058daf23281813b87a6b09fc95fadefc16ec85/fw-issues-20261012-x86/x86-record.txt

Not run: Linux, Windows and iOS hosts. The cause is in shared layout code, so they are expected to agree with macOS.
No macOS picture was taken: the agent's window screenshot comes back empty while the screen is locked.

## Constraints

- The web is the layout oracle (CLAUDE.md, "Write the web, ship the platform"). Chrome's column is the clamped
  item's height in every case above.
- The fix belongs in the vendored Taffy (`vendor/taffy/EXACT-PATCHES.md` numbers each divergence and records whether
  it goes upstream). Upstream main has the same code, so this is an upstream bug too, and a fix should go upstream
  with a Chrome-generated fixture, as patches 10, 12 and 14 were.
- Clamping a leaf's content-size measure in `compute_leaf_layout` would also make the four cases agree, but it would
  break the CSS distinction between a flex base size (unclamped) and a hypothetical main size (clamped), which flex
  resolution uses. The container's own sum is the narrower place.
- No open or closed issue under `issues/` covers it (searched `field-sizing`, `max-height`, `intrinsic`, `textarea`).
  GitHub #130 (closed) is about a hooked native text view's height, not this. Searches of DioxusLabs/taffy's issues
  and PRs (`max_size`, `max-height column`, `determine_container_main_size`, `flex_basis max_size definite`) found
  nothing on it.

## Acceptance criteria

- On macOS, cases A to D lay out the column at the clamped leaf's height (132, 132, 130, 132), so every row of the
  table agrees with Chrome in kind.
- Typing past a content-sized textarea's `max-height` in a column whose height is automatic does not grow the column.
- A kernel or vendored-Taffy test pins case C (a column in definite space, a measured leaf with `max-height`) with
  Chrome's numbers, and cases E to H keep their heights. The row-direction equivalent already holds: in the Taffy-only
  probe, an automatic-width row in definite space with a 656-wide leaf at `max-width: 130` is 130 wide.
- The T3 Code clone can drop X86's wrapper box.
