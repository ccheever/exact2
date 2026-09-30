# Symbol roles lack a list's common glyphs: bookmark, document, check circle, reorder handle, sort, filters

**Status:** Closed
**Resolution:** Fixed in a96e8094 Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Kernel (`kernel/tables/schema.json` `symbols`), every host's symbol rendering (SF Symbols on Apple, the SVG path elsewhere), Contract (`symbol:<role>`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** LLP 1035.004 D1 (symbol roles); issues/20260924-clip-path-subset-of-css.md (the same gap drove app artwork into `clip-path`)

Porting the Expo PR 49975 list demo (SwiftUI `List` + SF Symbols) to exact2 found no role for seven glyphs a list and its edit mode use. The port drew each one as a `clip-path` shape, so Apple showed hand-drawn icons where SwiftUI shows system ones:
- `bookmark` and its filled state (`bookmark`, `bookmark.fill`)
- a document (`doc.text`)
- a check circle, empty and filled, for selection (`checkmark.circle`, `checkmark.circle.fill`)
- a reorder handle (`line.3.horizontal`)
- sort (`arrow.up.arrow.down`)
- filters (`slider.horizontal.3`)

The table has 19 roles, each `[role, SF Symbol, SVG path]`.

**Fix:** add roles for these, each with its SF Symbol name and a 24-unit SVG path in the style of the existing entries.
- The filled states need the table's convention for a filled variant. Either a `-fill` role or a fill flag the hosts already honour: read how the existing roles handle fill before choosing.
- Keep the web and Linux paths visually close to the SF glyph.

**Done when:**
- `symbol:bookmark`, `symbol:document`, `symbol:checkmark-circle` and the rest compile.
- On iOS and macOS they render the SF Symbol.
- On the web and Linux they render the path.
- A parity case covers at least one new role.

**Fixed (a96e8094):** roles `bookmark`, `bookmark-fill`, `document`, `select`, `select-fill` (`checkmark.circle`, `.fill`), `reorder`, `sort`, `filter`. The fill convention: a filled state is its own role, `<role>-fill` exactly for Apple's `<name>.fill` and beside `<role>` (build.rs refuses either half alone); its browser path is a filled even-odd silhouette, marked `data-symbol-fill` on the web. Apple renders the SF Symbol, the web the path (`~/bench/listbench/symbols-web-gallery*.png`); listbench on iOS matches the Expo capture's glyph ink to the pixel (`~/bench/listbench/symbols-ios-{top,saved,edit}.png`). Still open: Linux paints no symbol for any role (LLP 1015 §7), and Done's `checkmark` has no role.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed in a96e8094
