# Code review: LLP 1121 stage 3, commit 1, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `1697a9f827c34da4b3ecc8d80018bce799f4ffa4ea1f0699ef260e57fd89a7a4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one medium, one low; Grok: one medium.
- **Disposition:** applied without re-review

---

The two quote fixes hold. One layout bug remains: an axis value named `under` cannot survive formatting.

**Medium** — `tamber/src/fmt.rs:153` (parser match at `tamber/src/syntax.rs:289`)

An axis value `under` that is not the last value parses in the tight spelling and then has no canonical layout. `tone=under|soft` and `placement=under|dim under scheme=dark` are legal: both sides of `|` are identifiers, and the clause is recognized only when the text after ` under ` tightens to `scheme=dark`. Formatting inserts the canonical spaces and produces `tone = under | soft` and `placement = under | dim  under scheme=dark`. On the way back in, `split_once(" under ")` now hits the value, the right-hand side is `| soft` or `| dim  under scheme=dark` rather than `scheme=dark`, and `format` returns `tamber-fmt` instead of a layout. The file is not written. The same call already rejects a hand-written canonical line, so that design cannot be represented in the layout this commit defines.

Treat `under scheme=dark` as a suffix (flexible spaces around `=`), not as the first ` under ` in the line, and use that split in both the parser and `axis`. `tone = under | soft` then stays values `under` and `soft`, and `placement = under | dim  under scheme=dark` keeps the clause.

**No high findings. No low findings.**

What round 2 changed is in place and behaves as described:

- `lex::words` keeps either quote, so `font.family = 'A x=y'` stays one value and is not metadata `x=y`.
- A `'` opens a string only at a token start (after a space, `=`, `(`, `,`, or `:`), in both `words` and `strip_comment`. `People's  // the face` keeps the comment, and the parsed value is `People's`. `\"…\"` and `\'…\'` still escape inside the string they opened.
- `split_top` still sees only `"`. On a line that parses, the structural `=` or `:` is the first one, and a comma inside parentheses is already protected by depth, so a token, dial, or `when` does not change. `unquote` still decodes only Contract `"…"` strings, so `label='My Label'` is stored with the quotes; that matches `unquote`'s own contract.
- The `scheme=fixed` paragraph is present in `docs/contract-for-agents.md` after the rebase.
- `when = #fff` is a token; a real `when` still has to be indented under one. Formatting again is stable for the layouts the tests cover: comment indent is preserved when the following line disappears, value spelling is kept, and `//` inside `'a//b'` stays. `fmt --check` writes nothing. The recipe file is about 2.4 KB, under its cap.

The fixed-point test counts `//` and compares the IR only when `check` already succeeds. It would not catch this `under` case, and the Bluesky fixture is not pinned to its formatted text (it also has no final newline).
