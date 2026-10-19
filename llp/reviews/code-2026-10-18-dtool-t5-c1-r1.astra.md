# Code review: LLP 1122 T5 step 1, the selection join's template and style rungs, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e3d912aff6`.
- **Method:** one brief (sha256 `3d9b78582aa46605f90c9758f8a5296b18ab045c145af536826b2208926b24be`), shared by both reviewers, blind to the other review; commits ae0b9b82c4 e3d912aff6. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 correctness; smaller points).
- **Disposition:** taken in `82d98133ff`: a state row written under the node takes the template rung (set-row at its place with `state`, its `row` the attribute its author wrote under the state, since the map gives a state binding no `attr`), held ones too; the value as written is read at the map's UTF-8 byte column with the attribute's name checked there, as Contract writes a plain literal (escaped strings decoded, a bare `#…` colour, an exponent, a minus, nested parentheses, a positional string after); `instances` counts call-site chains (a `button "…"`'s label is no copy); a written row carries `layout` in the selection too; a row set-row can't take (a layout row, one written otherwise, a package's file) still offers the token rung where its whole value is one token; the test's fixture is map-consistent (paint shorthand `border-radius`, layout rows marked and refused, each binding at its own attribute) with UTF-8, literal and label cases. Not taken: before the server's first file read the own-sources list is empty (T4's recorded window).

**Changes requested. Four correctness issues block sign-off.**

1. **Inline `:active` rows never reach the template rung.** [tamber-inspect.mjs:232](/tmp/rv-dt-t5r1/scripts/tamber-inspect.mjs:232) requires `b.attr`, but the compiler’s state bindings contain `state` and `at`, without `attr`. Consequently, `background-color="accent.pressed"` under `:active` still defaults to the **token** rung; a literal colour is refused, although `set-row` accepts its address with `state:"active"`. The [held-row path at line 338](/tmp/rv-dt-t5r1/scripts/tamber-inspect.mjs:338) also never calls `rowRung`, so supplying `attr` alone would leave held rows broken. Both paths need wiring.

2. **UTF-8 columns can produce another attribute’s value.** [tamber-inspect.mjs:245](/tmp/rv-dt-t5r1/scripts/tamber-inspect.mjs:245) applies the map’s byte column to JavaScript’s UTF-16 `slice`. For:
   ```contract
   text "你好你好你好" color="red" opacity=0.5
   ```
   the `color` rung returns `text:"0.5"` while retaining the colour attribute’s address. Shorter non-ASCII prefixes can instead make an editable literal appear read-only. Slice by UTF-8 bytes and verify the matched attribute name against `b.attr`.

3. **The literal reader refuses values `set-row` accepts.** The same [regex at line 245](/tmp/rv-dt-t5r1/scripts/tamber-inspect.mjs:245) rejects `opacity=5e-1`, `opacity=((0.5))`, and `text color="red" "Hello"` as expressions. Contract accepts these: parentheses disappear from the expression tree, exponents are numbers, and positional arguments may follow attributes. They pass [`plain`](/tmp/rv-dt-t5r1/contract/cli/src/tamber_rows.rs:89) and the single-line replacement logic. Escaped string literals are another excluded case. The reader needs to follow Contract’s literal and attribute-boundary syntax.

4. **Generated button labels inflate `instances`.** [tamber-inspect.mjs:324](/tmp/rv-dt-t5r1/scripts/tamber-inspect.mjs:324) counts every map site sharing the source span. For one `button "OK" color="red"`, the button and its generated text child share that span, yielding `instances:2`; two component expansions yield four. D6 requires “this template · 1 instance” for the single button. Exclude synthesized nodes when counting the addressed template’s expansions.

Two smaller points:

- [The protocol promises `layout:true` on selection rows](/tmp/rv-dt-t5r1/docs/tamber-protocol.md:1129), but [the join at line 351](/tmp/rv-dt-t5r1/scripts/tamber-inspect.mjs:351) omits it. The edit refusal works; either propagate the field or describe it as map-only.
- [The new test at line 356](/tmp/rv-dt-t5r1/scripts/tamber-inspect.test.mjs:356) still expects editable `padding` and `margin`, with their layout bits omitted. Its added width/gap bindings point at the padding attribute. Use a paint shorthand and map-consistent addresses so the test checks agreement with `set-row`.

The ordinary own-token default correctly becomes **template with token beside**. Param precedence, renamed styles in their declaring files, paint-shorthand addressing, ownership exclusions, and moved-file refusal passed the in-memory probes.

Validation: two existing filesystem-free tests passed, with 40 assertions, plus targeted in-memory probes. The file-writing fixture tests were not run. Nothing was changed.
