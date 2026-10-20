# Review round 2, LLP 1121 stage 11 B2 (the hosts' columns), Grok (grok 4.7, xhigh)

First pass (per-name design):

**RIGHT WITH CHANGES**

Round 1 is fixed on the paths it names. An SVG attribute alias writes the column length (`measures.rs:387`, `"16px"` in `$tla`; the static attribute stays the kernel's unitless length). `modes::structure` (`modes.rs:600`) appends the declared base names, and `sameShape` (`dev.mjs:408`) rebuilds when that string changes. `named` walks bytecode, `reads_length` probes the codec with `1px`, and the 40px→48px test sends a real `exact.design` batch to both the direct row and the alias. `emit.rs` is 1499 newlines; the cap fails when `split('\n').length` is over 1500, and this file splits to 1500. Columns: every JS build sets Neutral, wasm reads Neutral (space.xs 4 against iOS 5), the terminal calls `set_column(Terminal)` before boot, Linux sets Neutral, and the Android reader calls `show_material_look` (`jni.rs:144` and `:206`). Neutral and Material px values coincide; the Linux test separates them by source (`m3-spacing` vs `neutral-steps`).

The walk's region frames match the VM: depth 0 is the node's arm, and `when` counts as a frame, so an `each` item inside a `when` is `LoadItem(1)` and a `match` bound is the option's element. A number list declares none; a string list declares all nine. `reads_length` does not skip a row that would accept a resolved base length: `"1px"` and the neutral lengths (`4px`…`32px`, `9999px`) share the dimension path, and keyword and integer codecs refuse both.

Release output for the 14 named apps stays free of `--exact-` on inspection. None of them name a base, and every length binding there is a number, a non-base literal (`"100%"`, `"auto"`, `"0px"`), or a concat the walk correctly drops (`"px"`). `base_scale::css` is then empty and `emit.rs:765` appends it. I did not rebuild those apps against `d7eaafb72a`.

1. **SHOULD-FIX** `host/web-js/src/base_scale.rs:147` (`no_base`) and `:332` (`Concat`). A part disqualifies a concatenation when its raw text is not a substring of a base name, and that result then suppresses an unknown other side. The page looks names up with `v.trim()` (`measures.rs:415`), and the kernel's `is_base` trims too. `` padding=` ${gap}` `` or `padding=(gap + " ")`, with `gap` a string state an action can set to `"space.md"`, evaluates to `" space.md"`, which trims to a base name, and the walk returns no strings, so `--exact-space-md` is never declared. `" " + "space.md"` as a binary `+` (templates fold only when every part is a literal) is the same miss. `w.toString() + "px"` stays correct, because `"px"` is still not a substring after trim. Trim each constant the way the lookup does, and treat a part as disqualifying only when the trimmed text is non-empty and no base name contains it. Add that `gap + " "` case next to the existing `"px"` test.

2. **SHOULD-FIX** `host/web-js/dev.mjs:782` (`scrubShow`) and `:746` (`scrubPatch`). The file-edit path rebuilds. A design-tool scrub does not. Folding an unset alias keeps the bare name (`tamber/src/measure.rs:222`, `Length::Named`), `design-serve`'s `patched` (`main.rs:196`) stores that name as the theme value, and `measures::css` (`measures.rs:38`) turns it into `var(--exact-space-lg)`. `scrubPatch` then spreads the committed design, so the old `structure` rides along, and `scrubShow` compares shape and token names only before pushing the batch. Verify runs 150ms later and only for a patch (`scripts/tamber-scrub.mjs:156`), sees the real structure move, and restores. For that window the page references a variable the sheet never declared. Have the patch reply carry `modes::structure` of the patched plan, and have `scrubShow` refuse before the push when `structure` differs, the same way `sameShape` already rebuilds.

Re-review on the simplified build:

**RIGHT**

Round 1 is fixed, and the first round-2 findings are either dissolved by theme2’s all-nine ruling or fixed in `4c50cd1263` and `17285e4a0a`. No new defects.

Round 1: an SVG attribute alias is the column length in `$tla` (`measures.rs` map, `base_scale::pairs`); a live edit of an app-defined `space.md` moves both the direct row and the alias (40px to 48px in `base-scale-live.test.mjs`); declaration comes from length reads, so plain text declares nothing.

Round 2, as filed, then as they stand now:

1. Shared `$tl` for a keyword row and a length row. Fixed: when the sheet declares the scale, a row that admits a length token but whose codec rejects `1px` is `$tk` (`measures.rs` `map_name`).
2. `"space." + size + " "`, `" " + gap`, `gap + " "`. Dissolved: those declare all nine. `no_base` trims, and whitespace alone does not prove a concat base-free.
3. A bound SVG attribute left an alias at the old length. Fixed by the lead’s widening: a non-literal SVG length attribute marks every constant length token that row admits as inline, so the edit rebuilds and a scrub refuses it.
4. A scrub ignored structure. Fixed: `design-serve` returns `structure_of`, `scrubPatch` keeps it, and `scrubShow` refuses a difference (`dev.mjs`). Under all-nine, `space.md` to `space.lg` keeps the structure and is a batch; none to nine moves it.
5. `Some` / list elements in region frames. Fixed: the walk keeps `Value.item`, and `returns` counts that element.

The sheet is all nine `--exact-*` names the design does not define, or nothing (`base_scale.rs` `reads`). A defined name is left out, so `--exact-space-md` does not shadow `--x-space-md`. Static rows still bake the column length; bound CSS rows read `var(--exact-…)`; SVG attributes bake the length.

The bytecode walk does not miss a base name. A length row the walk cannot prove base-free declares all nine (`walk` returns `None` on an unknown opcode, a stack or local mismatch, or an untyped `LoadParam`). Concat is sound: a part is base-free only when its trim is non-empty and is not a substring of any of the nine, and the page trims the whole value, so a disqualifying character cannot be trimmed out of the middle. Region depth matches the VM (`frame_at`: depth 0 is innermost) and the compiler (`Scope::lookup` counts region frames only). An expression `match` is `JumpIfNone` (peek, as the VM does), `Unwrap`, and `BindLocal`; local indices line up when the binding starts at 0, and a higher start fails the lookup and declares all nine. `reads_length` probes `1px`, which takes the same codec path as this column’s `Npx` cells (dimension and line-height stay text; other pixel rows become a number first). Keyword rows fail that probe and declare nothing.

Release output for an app that reads no base name stays byte-identical with pre-B2: `css` is empty and map names stay on the old `$t…` spelling while `bases` is false. The commit records 14/14 against `d7eaafb72a`. I did not rebuild those apps. An untyped parameter or an unhandled `Record` / `Map` / `Filter` on a length row would declare all nine; that is the conservative rule, and the claim says none of the 14 hit it.

Hosts read the columns the ruling names. The JS target and wasm set the neutral role, which is the metrics column; the web test separates neutral `space.xs` (4) from iOS (5). The terminal sets `Column::Terminal` before boot (cells, radius 0, not 9999). Linux `show_neutral_look` and the Android reader’s `show_material_look` are what the tests call; neutral and Material px steps coincide, and the test separates them by source (`neutral-steps` vs `m3-spacing`) and by `radius.md` through `theme::measure`. `emit.rs` is at the 1,500-line cap (`split('\n').length` is 1,500), not over.

No findings.
