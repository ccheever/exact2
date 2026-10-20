# Code review: LLP 1122 T5 step 2, the tool's set-row editor, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `e3d912aff6`.
- **Method:** one brief (sha256 `38c94878355dfcb3ec69a43a4d4adc480b3e226a694d9004da21992c59f75c94`), shared by both reviewers, blind to the other review; commits f0f5e10a2a. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (smaller points).
- **Disposition:** taken in `82d98133ff`: an edit of the selected copy's source (set-state, set-row, promote, detach) is refused at propose when the files the proposal reads are not the ones the selection was reported under, so a draft opened on old files can't be applied over a newer write in flight (T4's path too); the editor's heading names the attribute written, a shorthand's whole ("This template · 1 instance: border-radius"); the copy's key includes the build's facts (its pin, reload or plan), so a draft opened before a pin stays stale after reselecting; the undo words name a style's file; the test's style row is a paint row in its own file. Not taken: asserting the outgoing op's JSON (the contract DSL compares whole texts and has no escaped quotes; the op's fields are the join's address passed through, checked by the join tests) and a real compile through the stub (the gate runs the server).

Nothing blocks. Commit `f0f5e10a2a` sends a `set-row` the server can apply, binds the draft the way the state editor does, and leaves a refusal unwritten.

## The op

`rowOf` keeps only the address the join gave, and `opOf` spreads that JSON and sets `value` after it (`apps/design-tool/inspector.ts:55-57`, `apps/design-tool/app.ts:238-239`).

A template row whose text is the token `surface.card`, at `card.contract` line 3 column 9, goes out as:

```json
{"op":"set-row","file":"card.contract","at":{"line":3,"col":9},"sha256":"<the join's digest>","row":"background-color","value":"surface.card"}
```

`surface.card` stays a string. `NUMBER` (`app.ts:246`) is the same test `set-state` already used, and it does not match a token name. The server then writes a Contract string (`tamber_contract.rs:189-194`, `string_literal`). `at.line` and `at.col` stay numbers, which `set_row` reads with `as_u64`. `state` is added only when the join sent one (`inspector.ts:56`), which is the inline state part under the node.

A style row `padding` whose text is `12`, then edited to `16`, goes out as:

```json
{"op":"set-row","style":"Panel","row":"padding","file":"card.contract","value":16}
```

`16` matches `NUMBER`, so the value is a JSON number and the server writes the Contract number `16`. `file` is the join's `file`, the style's own file (`rowRung` sets it from the attribute's file). There is no `at` and no `sha256` on that op, which is the style form in the protocol. A param rung never reaches `rowOf`, so it stays on `set-param`.

## Staleness

Two checks, and both leave the file alone.

The draft is bound like T4. `editRow` stores `` `${inspected.key}␟${filesNow}` `` (`app.contract:877`), and `stateStale` is the state-editor condition with `set-row` added (`app.contract:699`): a stale selection, a read-only selection, a file digest that has moved, or a different copy. Apply requires `not stateStale` (`app.contract:1579`). Open the template editor, then have another write change the poll's digests: the orange line shows and Apply stays off.

The address carries the compile digest. Propose and the transaction both send that `sha256`. A mismatch is a 409 with diagnostics (`tamber_rows.rs:212-220` wrapped at `tamber_contract.rs:579`). `refusal` puts the message and each diagnostic's file, line, id, and text on the proposal (`app.ts:44-46`, `app.ts:824-825`). `proposal.ok` is false, so Apply is off, and `writeEdit` pushes an undo only when the server returns an inverse (`app.ts:850-856`).

A style op has no `sha256`. Its guard is that same draft binding, plus the transaction's file digests and the compile. That matches a name-addressed style edit.

## The state editor, the branch editor, scrubs

`set-row` reuses the state editor: same field, same trim, same stale line (`app.contract:1544-1554`, `stateValue` at `882`). `editRow` sets `editKind` to `set-row` and clears `selToken`, so the branch panel closes (`app.contract:871`, the branch panel is `selToken != ""` at `1464`). Opening it while a scrub is up cancels that scrub first (`867-870`), the same as `editState`.

The chip is the server's class. The stub marks every `set-row` `rebuild` (`stub/tamber-stub.ts:46`). Scrub live is only offered for a dial, a token, or a param (`app.contract:1581`), so this editor has no scrub.

The token stays beside the default. For a template or style rung with a token, the row gets "Edit the token … instead" and a separate "Edit this template · N instances" or "Edit style …" (`app.contract:1289-1300`). One instance is labelled "1 instance" (`inspector.ts:57`). A layout row or a conditional class arrives as `rung: null` or as the token rung, so `rowOf` offers no `set-row`.

## Labels and undo

Undo is the server's inverse, sent back as `undo`, not a second `set-row` (`app.ts:953-957`). The stack label is `background-color at card.contract:3 = #ff0000` or `Panel padding = 16` (`app.ts:247-249`). The proposal wording quotes a string value and leaves a number bare, and it appends the file (`app.ts:584`, `590`).

## What the test locks

`app.test.contract:1415-1455` locks the two buttons and their words, the field starting at `surface.card` and at `12`, Rebuilds with no Scrub live, Apply advancing undo from 1 to 2, and the two stub refusals disabling Apply. A bad colour is refused before the good `#ff0000` write, so that refusal did not become an undo.

It does not read the op. The stub's diff is `JSON.stringify` of the op (`tamber-stub.ts:207`), and the test never checks `proposal-diff` or `editTarget`. Dropping `sha256` or `col`, or sending `16` as a string, would still pass. It also does not replay T4's "someone else wrote the file" or "inspect turned off" cases; those sit on the shared `stateStale` derive. The style fixture uses `card.contract` for both rows, so a style that lives in a second file is only implied by passing `editable.file` through.

## Smaller points

The style undo label names `Panel` and the row, and not the file. The op does carry `file`. Two files that each declare `style Panel` would read the same in "Can't undo …".

A quoted numeric literal and a bare number arrive as the same `text`. `padding="12"` would be sent as the number `12`, the same rule as `set-state`. A token name does not take that path.

The stub's refusal is a flat `message` for the sentinels `sha256 === "stale"` and `value === "not-a-colour"` (`tamber-stub.ts:45`, `203-206`). A real compile failure is `{message: "refused", diagnostics: [...]}`, and `refusal` already prints those diagnostics. The new test does not drive that shape.

I did not run the contract test.
