# Review round 1, LLP 1121 stage 9 icon-alias fix, Grok (xhigh)

Reviewed: 3fac27b365 (before the amend that changed the corpus line; see round 2).

## BLOCKING

**`refuse-icons` no longer matches its golden file** — `tamber/src/icon.rs:125`, `tamber/tests/corpus/refuse-icons.err:10`

`aliased` copies an icons-block value before `check` looks at names. In `refuse-icons.tamber`, `icon.bare = star` (line 22) names `star` (line 23). `star`'s value is the valid literal `icon(sf "star")`, and its name is refused (`tamber-icon-name`). After the copy, `icon.bare` is a valid icon, so the `tamber-kind` at 22:15 is gone. `refuse-icons.err` still expects that line, and `tamber/tests/it/corpus.rs` compares the full text, so the corpus test fails. The right-hand side was not changed to `sparkle`.

The design is still refused, on `star`. An undotted name still cannot survive in a design that checks. Taking `star`'s sources matches the new rule (any icons-block row is a target). That row no longer witnesses "a value that is not `icon(...)`". Drop the stale `22:15` line, or point `icon.bare` at a name that is not an icon (`sparkle`) if that witness should stay.

## SHOULD-FIX

**An alias to a bad `icon(...)` is diagnosed as if that literal were written on the alias** — `tamber/src/icon.rs:127`

The copy keeps the alias token's line and column (`icon.rs:185`) and replaces its text. For `icon.alias = icon.bad` with `icon.bad = icon(png "x")`, both rows report `` `png`: an icon's sources are `sf`, `material` and `svg` ``. On the alias line the source text is `icon.bad`. `check` used to say `an icon is icon(...)` there, which is the text on that line. Unknown names and cycles keep their written text, so those diagnostics still match the span.

## What checks out

- A valid chain resolves in `check_design`, so it resolves for every fold that goes through `check` or `check_design`: CLI `check`, `eval`, `ir`, and `export dtcg`; the server's `design()` export; the scrub's role-variance guard (`tamber_scrub.rs:131`). `laid` still calls `aliased`; the second pass is a no-op once values are `icon(...)`.
- An alias to no icon, a cycle, or a theme token that is not an icon stays unresolved (lookup is icons-only) and is `tamber-kind` at that value's own line and column. Colour errors and icon errors are merged and sorted by line, then column (`lib.rs:191`).
- A name that is both a theme token and an icon: the alias copies the icons-block value; the icon row is `tamber-duplicate`; the design is refused.
- `folding.design` now stores resolved icon text on every successful fold. Readers that need the written alias do not use it. Explain re-parses the snapshot (`tamber_explain.rs:36`, `79`). `reads` and `ir`'s `components` take the parsed design (`tamber_params.rs:108`, `160`). History compares parses (`history.rs:6`, `159`). Edits rewrite the parse. `fmt` formats source. DTCG skips icons. Session `shape` compares resolved `Icon` sources (`session.rs:410`). Theme retention scans the design after `add_to`, which already ran this loop inside `laid` before this commit.

## Tests

`an_icon_naming_another_checks_as_that_one` proves a chain of two, an unknown name, and a cycle (ids only). `an_icon_alias_reads_without_params` proves `check`, `eval`, and `export` with no params. `an_icon_alias_builds_beside_params` does not prove this commit: that path already resolved aliases in `laid`, and the assertion is only that one plan string exists.

Untested, and worth a case each: an icon naming a theme token, a name that is both, the alias-of-a-bad-literal diagnostic above, and the `icon.bare` / `star` choice once the golden file is fixed. `ir` and the scrub share `check` with the CLI test.

After round 1: the corpus line was already `sparkle` in the amended commit; an alias to an icon whose sources don't read keeps its own text (`aliased` copies only a source that reads), in round 2's sha, 4ed1b22ed4.
