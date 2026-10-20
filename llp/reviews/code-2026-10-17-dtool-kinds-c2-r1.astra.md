# Code review: LLP 1122, stage 6 kinds in the tool, step 3 (typed fields by kind), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `69ea44a2b`.
- **Method:** one brief (sha256 `74a8aacc917645e431ee8bd95d756de5ba1f5c2cef977fa49675143e2a577f64`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 5 medium).
- **Disposition:** taken. Steps 3 and 4 are now one commit (`056c17479`), which round 2 reviews.
  - Typing: the fields are cut from the branch's value, or the text field's, never from the draft. Each field's text is state for that cut, so an incomplete part moves no field and loses no focus. This also fixes the lag (a second edit composed from parts that predate the first).
  - The numeric grammar is shadow.rs's: a sign, an exponent, any dimension or number is a length. `inset` is matched in any case.
  - A required part left empty drafts nothing (an empty value is proposed as "Give it a value"), and the editor says which part.
  - The type grammar is typeface.rs's: `italic` only before a weight, any number as the weight, a whole-value name of a design token gives no fields, and `normal` or `oblique` gives no fields.
  - A value naming a token keeps its links ("Names …"), with no "Pinned here".
  - Tests: incremental typing with no answer between, a required part emptied, only this mode, a typed scrub, and real-server one-line writes for a length, a number, a box-shadow list, a text shadow and a type longhand.

- **High — Ordinary typing destroys the editor.** [app.contract:1140](/tmp/rv-dt-k3/apps/design-tool/app.contract:1140), [app.contract:1160](/tmp/rv-dt-k3/apps/design-tool/app.contract:1160): entering `1px` into the empty spread field first produces `1`. Reparsing that draft returns `ok=false`, removing the fields and focus before `px` can be entered. An opening quote in a family or parenthesis in a colour has the same problem. **Fix:** retain the editing parts and incomplete field text; validate the assembled value without rebuilding the field structure after every character.

- **Medium — Pending parts can overwrite another edit.** [app.contract:708](/tmp/rv-dt-k3/apps/design-tool/app.contract:708): both rendering and `partEdit` trust `typed.ok` without checking `typed.of` against `shownValue`. If amount changes from `12` to `16`, then unit changes before the resource settles, the second composition produces `12rem`, losing `16`. **Fix:** compose from synchronously updated draft parts and require matching source/branch identity when adopting resource results. Add a test that holds the resource reply between edits.

- **Medium — A valid shadow length can become the colour field.** [parts.ts:12](/tmp/rv-dt-k3/apps/design-tool/parts.ts:12), [parts.ts:87](/tmp/rv-dt-k3/apps/design-tool/parts.ts:87): Tamber accepts `text-shadow = 0 1px +2px`, with blur `2px`. The editor labels `+2px` as colour; changing “colour” to `red` produces valid `0 1px red`, silently removing the blur. Exponent notation also shifts field assignments. **Fix:** match Tamber’s numeric grammar, including leading `+` and exponents, before assigning shadow roles.

- **Medium — Clearing a required shadow component changes another component’s meaning.** [parts.ts:106](/tmp/rv-dt-k3/apps/design-tool/parts.ts:106), [app.contract:713](/tmp/rv-dt-k3/apps/design-tool/app.contract:713): clearing blur in `0 1px 2px 3px red` produces `0 1px  3px red`. Tamber accepts this with blur `3px` and no spread. `optional=false` does not preserve the positional slot. **Fix:** retain an invalid field draft and prevent proposal/scrub submission while required slots are empty.

- **Medium — Type fields disagree with the server’s grammar.** [parts.ts:150](/tmp/rv-dt-k3/apps/design-tool/parts.ts:150): Tamber reads `600.0 16px` as weight and size; the editor reads it as size and family. Editing “size” to `700` therefore changes weight. Whole-token aliases such as `type.body` are also incorrectly presented as size fields. **Fix:** recognize the server’s accepted weight spellings and distinguish aliases using snapshot kinds. Match its ordering rule too: the test’s `600 italic 16px` is rejected by `typeface.rs`.

- **Medium — Typed expressions are falsely reported as pinned.** [app.contract:1122](/tmp/rv-dt-k3/apps/design-tool/app.contract:1122), [app.contract:1138](/tmp/rv-dt-k3/apps/design-tool/app.contract:1138): merely selecting the fixture’s `shadow.ring` hides its dependency controls and says it no longer follows `surface.inset`, although the reference remains unchanged. **Fix:** retain dependency navigation alongside typed fields and distinguish a part edit from an explicit pin.

Unchanged recognized values round-tripped exactly in the read-only probes, including whitespace, nested functions, `light-dark()`, quoted families and comma-separated stacks. Snapshot kinds correctly include length dials. Scope selection, stale-draft fallback, single-operation scrub routing and token/shadow-specific aria labels look consistent.

The tests **do not prove every binding condition**. [app.test.contract:107](/tmp/rv-dt-k3/apps/design-tool/app.test.contract:107) checks drafts using complete-value inputs and settlement between edits. [design-tool.test.mjs:1130](/tmp/rv-dt-k3/scripts/design-tool.test.mjs:1130) checks actual one-line writes only for box-shadow and type. Add length, number, text-shadow, typed scrub, mode-scope and incremental-input cases.

The stub appears faithfully regenerated: its source SHA-256 and reconstructed IR digest match, and all 21 tokens have resolve/explain entries. I ran only in-memory checks; the integration tests write files and were not run.

**Verdict: Request changes — normal typing and valid grammar cases can remove fields or edit the wrong component.**