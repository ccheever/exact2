# Code review: grouped lists read a row's symbol however it's given (stage 13 C fix 1, theme2 16:37), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b0413f0d90`.
- **Method:** one brief (sha256 `611b9dff5e8c149bd66dcb521b3f97888353acf0f227b70487614949c342b5b4`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken: the model's action and symbol are read

**VERDICT: RIGHT WITH NITS**

The placeholder `"symbol"` is only a classifier token. No caller writes it into the image source, the plan, or the kernel. A template whose runtime value is a chevron or checkmark is safe to treat as "not an accessory." `leading()` is right on the edge cases you listed. No app, example, or fixture in this tree changes.

1. **Nit — the new test pins sheet ink and icon size, and not the native action bit or the image source.** `contract/cli/tests/it/grouped_list.rs:1539`

`colour("{t}-title")` is the computed title color, so written, named, and shown are checked against the accent, and `colour("push")` is checked against `-exact-label`. Frames compare named and shown icons to the written icon (`grouped-symbol-width` / `grouped-symbol-height`, 24×20). That is the bug (label ink instead of the accent) and the symbol metrics.

It never reads `GroupedRow.action`, which is what a native list uses. The sibling test does, at `contract/cli/tests/it/grouped_list.rs:1441`. It also never reads `ImageSource`, so a later edit that wrote the placeholder `"symbol"` out as the Apple name would still pass: explicit width and height do not depend on the name. For these fixtures the bit is set and the source stays the real symbol (`symbols::roles` folds a constant template when the image is bound, after classification). Worth one assert on each.

**Callers of `symbol()`.** The returned name is used only as a yes/no or as an accessory / `info.circle` test. `"symbol"` is neither an accessory (`accessory()` is only `chevron.forward`, `chevron.right`, and `checkmark`) nor `info.circle`.

| Caller | What the placeholder does |
|---|---|
| `actions.rs:34` `leading` | Non-accessory symbol. This is the action-cell fix. |
| `actions.rs:45` disclosure | Not a chevron, so no disclosure tint. Same as before, when a template returned `None`. |
| `grouped.rs:884` inset | Uses `row-inset-symbol`. Same choice as a literal non-accessory symbol. |
| `grouped.rs:1216` `part` | Symbol width, height, pull, gap, and tint on the image node. `with()` keeps the template positional. |
| `grouped.rs:1228` trailing accessory | `accessory("symbol")` is `None`, so no chevron or checkmark metrics. Unchanged versus `None`. |
| `grouped.rs:1316` and `:1439` info button | Not `info.circle`. Unchanged. |
| `grouped.rs:1431` `text_stack` | A template at the leading edge now counts as the subtitle cell's symbol. |
| `title.rs:31` and `:43` | `is_some()` only, so a template now qualifies a title button the way a literal symbol does. |

The kernel reads `ImageSource` in `apple_symbol` (`kernel/src/grouped.rs:228`), not this token. A constant `` `symbol:${"add"}` `` is still folded to the real role when that prop is bound (`contract/lower/src/symbols.rs:60`).

A template chevron or checkmark stays "not an accessory" on purpose (`grouped.rs:641`). That matches the kernel: a leading Apple name, including `chevron.forward`, is `out.symbol`, and only the trailing child is an accessory (`kernel/src/grouped.rs:412`). A trailing template was already not styled as an accessory, because `symbol()` used to return `None`. Literal chevrons and checkmarks are unchanged. Nothing in `apps/`, `examples/`, `scripts/fixtures/`, `docs/patterns/`, or `contract/corpus/` puts a `` `symbol:…` `` template in a grouped row. The if-wrapped symbols that do exist are trailing literal checkmarks (`scripts/fixtures/grouped-list.contract:92`, `contract/lib/content.contract:322`), and those still take the literal path.

**`leading()` (`actions.rs:22`).** `else if` is an `else` whose one statement is the next `if` (`contract/syntax/src/parser.rs:939`), so the otherwise arm is a single nested `When` and recursion handles it. Both arms empty is a leading symbol, so `[empty if, text]` is an action cell, which is a title-only row. A hidden child is `display="none"` (`grouped/inputs.rs:83`) and is dropped before the arm is counted. An arm whose one node is a text fails `symbol()`, so the row is not an action cell. A `for` (`Node::Each`) is not walked; `symbol()` is `None`. That matches `part()`, which clones an `Each` without styling its body (`grouped.rs:1203`). A nested `if` is one shown node that is itself `leading`.

**Other apps.** No output change. Searched `apps/`, `examples/`, `scripts/fixtures/`, `docs/patterns/`, and `contract/corpus/` for `` `symbol: `` and for `if` arms that contain `image "symbol:`. The only template sources are tests and `host/web-js/conformance/symbols.contract:44`, which is not a grouped row. Shelf's stars and AnswerTile's marks are ternaries, which `symbol()` still does not read.

**Style.** Comments, the long test name, and `NAMED_SYMBOL` match the surrounding files.
