# Code review: grouped lists read a row's symbol however it's given (stage 13 C fix 1, theme2 16:37), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b58bfdb013`.
- **Method:** one brief (sha256 `9d5bbd97f981b4fd08955060076626d22bd99d2dd2c92feee40d9d4a6db0c2d0`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**VERDICT: RIGHT**

No findings.

1. **Callers of `symbol()`.** The new `Some("symbol")` is only a classifier token. `part` (`contract/lower/src/grouped.rs:1216`), `inset` (`:884`), `text_stack` (`:1431`), and `title_button` (`contract/lower/src/grouped/title.rs:31`) now treat a `symbol:` template as a non-accessory leading symbol, so it gets the same width, height, pull, gap, tint, and `row-inset-symbol` as a literal icon. That is what the icon-frame check is pinning. `accessory()` (`grouped.rs:677`) only matches `chevron.forward`, `chevron.right`, and `checkmark`, so a template still does not become a disclosure, a checkmark, or an info button (`:1228`, `:1316`, `:1434`, `actions.rs:45`). Nothing writes the token into the image source; the kernel still reads the evaluated `symbol:…` string (`kernel/src/grouped.rs:228`). A template that happens to be a chevron or checkmark at run time stays a leading icon when it sits in the leading slot, which is the same call the kernel makes (an accessory is only the last child). A trailing template chevron is unchanged: it was already not an accessory in the sheet.

2. **`leading()` (`actions.rs:22`).** Reading `Node::When` is right. An empty arm counts as no symbol, so `if symbol != ""` with no `else` is an action cell in both states. A nested `if` and an `else if` are one `When` whose arm is another `When` (the parser nests `else if` at `contract/syntax/src/parser/view.rs:16`), and the function recurses. A `display="none"` sibling is dropped with the same `hidden` used for the row, so one real symbol remains. A `for` is an `Each`; `symbol()` is `None`, so the row stays a normal cell (a `for` can show many symbols, and the ink is one static color). A text arm fails `symbol()`, which is the `worded` case. A `match` is also refused, in line with `part`, which already refuses to treat a symbol in a `match` as leading (`grouped.rs:1183`).

3. **Other apps.** No grouped row in `apps/`, `examples/`, `scripts/fixtures/`, or `docs/patterns/` uses a `` `symbol:…` `` template or a leading `if` around a symbol. The `if` + `symbol:checkmark` rows (the grouped-list fixture, `AnswerTile` in `contract/lib/content.contract`) put the mark after the title, so `action_cell` still declines them. Ternary sources such as Shelf’s stars never went through this match.

4. **The test** (`contract/cli/tests/it/grouped_action.rs:119`) pins the three claims: `row().action` and the title’s color against the accent sample for literal, prop template, `if`, and `if` plus template; icon width and height against the literal icon; `push-title` (the text, not the row) stays the label while `row().action` is false. `row().symbol` is compared with the literal row, so a kernel stand-in `"symbol"` fails. After the press, `shown` and `both` stay action cells with no symbol. `worded` and `pair` are refused. `grouped_list.rs` is 1,496 lines; the new module is registered from `main.rs`.

5. **Style.** The comments and the long test name match the surrounding grouped-list code. `leading` and `NAMED_SYMBOL` sit in the same vocabulary as the sheet.
