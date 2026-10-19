# Code review: LLP 1122 T5 step 1, the selection join's template and style rungs, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `61bc295299`.
- **Method:** one brief (sha256 `5b87dfbc8eb802f9d73cf36e37ab3a52ad380fb58f76e833c0cc28f6ef4ba2d9`), shared by both reviewers, blind to the other review; commits 82d98133ff. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 correctness; smaller points).
- **Disposition:** taken in `bc485dd65f`: a held look's row is set-row's only where it is written under the node in that state (a hover's row with no place of its own is the base binding, so no template address points at the base), its `instances` the held node's own call-site chains, an unread param still refusing it; a string's escapes are decoded one by one as the lexer does (`\n \t \" \\ \` \$ \u{…} \uXXXX`; another refused); a positional argument in parentheses or a template after a plain value; the fixture's rows by their wire names (`border_radius_top_left`, `border_color_top`); the held inline check passes `app` and asserts the template rung; the doc says the state rung's text is the plain value's and a layout row still offers its token. Not taken: `#12`/`1.`/`.5` accepted by the reader but not the lexer (a built map never carries them).

**Request changes. Four correctness issues remain.**

1. **Held hover/focus edits target the base row.** [tamber-inspect.mjs:366](/tmp/rv-dt-t5r2/scripts/tamber-inspect.mjs:366) now passes the held row through `rowRung` using the ordinary binding with that name. For:
   ```contract
   button background-color="blue"
     :hover
       background-color="red"
   ```
   the held `background_color` reports red, but its template address points at the base attribute, with `text:"blue"` and no `state`. Applying the edit changes the base and leaves the hovered appearance unchanged. The template address must belong to the held state; hover/focus still lack that provenance.

2. **`plainAt` does not decode Contract strings faithfully.** [tamber-inspect.mjs:274](/tmp/rv-dt-t5r2/scripts/tamber-inspect.mjs:274) replaces Unicode escapes before JSON decoding, including sequences behind an escaped backslash. I reproduced:
   ```contract
   marker-end="url(#a\\u{62})"
   ```
   becoming text containing a **backspace**, instead of the literal `\u{62}` sequence. The template editor therefore starts with a different value. Valid Contract escapes also remain refused: `marker-end="url(#a\$b)"` and `color="red\u{9}"` return null. Contract’s [escape decoder](/tmp/rv-dt-t5r2/contract/syntax/src/lexer.rs:500) accepts both. Decode escapes sequentially using Contract’s rules. This reader also supplies T4’s state text.

3. **Valid positional expressions still suppress the template rung.** The boundary check at [tamber-inspect.mjs:280](/tmp/rv-dt-t5r2/scripts/tamber-inspect.mjs:280) accepts a following quoted string, but rejects:
   ```contract
   text opacity=0.5 ("hello")
   ```
   This exact form already has a successful `set-row` proposal assertion in [tamber_serve_rows.rs:412](/tmp/rv-dt-t5r2/contract/cli/tests/it/tamber_serve_rows.rs:412). The attribute remains one plain number; the parenthesized positional is separate. Likewise, `text color="param.ink" ("Hi")` keeps the param rung but loses its `template`, disabling detach. Round 1’s parser-parity finding is only partly fixed.

4. **Held inline param rows bypass the unread-param refusal.** At [tamber-inspect.mjs:366](/tmp/rv-dt-t5r2/scripts/tamber-inspect.mjs:366), `unread` is always passed as `false`; scrubbing the held report has already discarded that flag. While param metadata is loading, an inline `:active` row containing `"param.fill"` is correctly refused in the ordinary rows, but its held counterpart now offers a **template** edit. I reproduced both answers from the same selection. Preserve the unread flag so the held path respects the same refusal and param precedence.

Smaller points:

- **Held instance counts use the selected descendant’s template.** [tamber-inspect.mjs:352](/tmp/rv-dt-t5r2/scripts/tamber-inspect.mjs:352) computes `copies` for `node`, then passes it to the held owner’s rung. One button containing a `Label`, with another `Label` elsewhere, reports two instances for the button’s held template. Count the owner’s call-site chains separately.
- **The revised fixture is still not fully map-consistent.** [tamber-inspect.test.mjs:337](/tmp/rv-dt-t5r2/scripts/tamber-inspect.test.mjs:337) uses `border_top_left_radius`; the wire name is `border_radius_top_left`. Its `border_top_color` is likewise `border_color_top`. The held-inline check at [line 107](/tmp/rv-dt-t5r2/scripts/tamber-inspect.test.mjs:107) omits `app` and never asserts editability, so it does not verify the newly wired template rung.

The other taken points check out: UTF-8 columns and attribute-name checking; ordinary and held inline `:active` addresses; exponents, negatives, nested parentheses and bare colours; generated-label deduplication; selection-row `layout`; token fallback; and known-param precedence with a template address. Removing T4’s bare-name prefill is justified because styles require literals. Deferring the empty own-sources startup window is reasoned: it is recorded under T4, and the writer still enforces file ownership.

Validation: two existing tests passed **40 assertions**, and **51 additional in-memory assertions** passed. Targeted probes reproduced the findings above. File-writing tests and Rust builds were not run. Nothing was changed.
