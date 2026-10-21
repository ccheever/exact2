# Code review: LLP 1121 stage 3, commit 2, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `c0c0579b23b00be20245cd7735d2b83769b5eccc1b7497b0926aa1c5326f0700`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, six medium, one low; Grok: one high, four medium, one low.
- **Disposition:** fixed in round 2; named fields on the wire are commit 3's

---

**High** — `Op::Restore` is a general write, accepted on the same path as client operations. `tamber/src/edit.rs:347`

`apply` and `apply_all` take every `Op`, and `valid` only rejects a `Restore` line that contains a newline (`edit.rs:259`). The splice at `edit.rs:347` then replaces any in-range span and sets the file's trailing newline. `check` still has to pass, so this is not an arbitrary byte string, but it is any design the checker accepts, plus bytes the checker does not look at. One operation can insert a comment, flip only the final newline, add or remove a dial, set `under scheme=dark`, or rename a role override by rewriting its line. None of those are in D13's write table. Commit 3's transactions return this enum as the inverse, so a client can submit a forged `restore` for a digest it knows and write the file.

Keep `restore` off the client write schema. Apply it only through an inverse path, and if a client resubmits an inverse, accept that `restore` only when it is byte-identical to one the server issued for that digest.

**Medium** — A value containing `//` or a trailing `name=value` word is pasted into the line and then re-lexed. `tamber/src/fmt.rs:232`

`valid` only requires the value to be one line (`edit.rs:212`). `token_with` and `dial_with` write that string as-is and then append the line's existing metadata, `in` range, and comment. The lexer cuts a top-level `//`, so setting `#000000 // note` on `ink = #ffffff  scheme=fixed` stores `#000000`, drops `scheme=fixed`, and still checks. The same paste turns a value `#fff  scheme=fixed` into real metadata. A dial value `2 // x` drops its `in` range and `step`.

Reject a value whose parsed form is not exactly the requested text with the line's metadata and range unchanged. `//` inside parentheses or a string, as in `url(http://x)`, stays valid.

**Medium** — A quote anywhere lets one metadata field carry several words. `tamber/src/edit.rs:231`

The check is `!ident=value || (has space && no quote character)`. `label="Ink here"` is a single word and should pass. `label="Ink here" scheme=fixed` also passes, because of the quote, and `AddToken` writes both words (`edit.rs:490`). The parser stores both, so one field can attach `scheme=fixed`. Rename is unaffected: it uses `lex::words`, which keeps a quoted value together.

Require exactly one `lex::words` word, a `name=value` shape, and a successful `unquote`.

**Medium** — A role override can be renamed whenever the design does not already check. `tamber/src/edit.rs:135`

The guard uses `check_design(...).ok()`, so any existing diagnostic skips it. On a design whose only error is `ink = missing`, renaming `-exact-label` to `missing` makes the reference resolve and the final `check` pass, and the role token is renamed. When the guard does fire, the diagnostic is still `1:1` (`refuse`), not the token's line.

Decide "this name is a role" from `vocab` and the token's spelling, whether or not the rest of the design checks, and point the refusal at that token's line.

**Medium** — Rename still rewrites colour-function syntax, not only token uses. `tamber/src/edit.rs:788`

A match is skipped after `#`, before `(`, inside quotes, and in metadata. It is not skipped for the words those functions read as syntax. `oklab`, `srgb`, `display-p3`, `lab`, and `shorter` are not reserved token names (`exact_color::parse("oklab")` is `None`). Renaming a token `oklab` to `srgb` turns `color-mix(in oklab, ink 50%, white)` into `color-mix(in srgb, …)`, which still checks and mixes in a different space. `shorter` to `longer` does the same for a hue method.

Rename only identifiers the value parser resolves as a token, dial, or role. Leave the space and hue words that `color-mix` and `color()` consume as text.

**Low** — The public operation types are anonymous tuples, which is the JSON shape commit 3 will serialize. `tamber/src/edit.rs:21`

`WhenLine` is `(conditions, value, comment)` and each condition is `(String, String)` (`edit.rs:40`). A derived encoding is positional (`["dark-style","black"]`, and a when-line is a 3-array). D13's operations are named fields (`set-token`, `axis`, `value`). `SetToken.at` is also on that struct; it is only an insertion index.

Use named structs (`{axis, value}`, `{when, value, comment}`) and the table's kebab-case operation names, with `restore` absent from the client write schema.

## What holds

The prefix/suffix diff is a real inverse, including with repeated lines. The suffix is taken from the remainder after the prefix, so the two spans do not overlap, and splicing `a`'s span back over `b`'s span returns `a`. `apply_all` inserts each step's `Restore` at the front, so they run last-first against the text that step saw. The round-trip in `tamber/tests/edit.rs` covers that order.

`edit.rs` is 801 lines and `tests/edit.rs` is 501, both inside the 1,500-line cap. There is no separate cap on how much text one `Restore` may carry; that is the high finding, not an extra one.

No other high, medium, or low findings. The tests never exercise a value containing `//`, a metadata field with a quote and a second word, a role rename on a design that does not check, or a token whose name is a colour-space keyword.
