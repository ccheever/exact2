# Code review: LLP 1122 T3 steps 2-3, the component panel and the param rung, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ebafe6aac`.
- **Method:** one brief (sha256 `c7862823cd23b256e5b21de922c7e504cfe7e4fe7c55b7cd31774ffd1dc0a7eb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** request changes (1 high, 1 low).
- **Disposition:** all taken in `ee057e3e2`: Apply writes the operations it reviewed (kept in the review), and `opsOf` reads the newest settled read of the app's design, never a missing one; that two-operation edit offers no scrub (`proposal.scrubbable`); a duration's select or field keeps its values as written, only its slider or stepper strips and writes ms; Only this mode after a panel edit starts its own branch draft; a branch editor edit (its value, a field, Only this mode) ends a scrub on another branch or the panel's; the panel shows on the app's design named by its file too. A contract test drives Only this mode on a param with no line (it fails with the scrub left on).

This blocks. On a settled design read, "Only this mode" for a param with no line does write that line first, and one undo takes both lines back. For the whole of any design read, that prepend is dropped and the server refuses the `when` line again.

## Blocking

### [P1] A design read hides the snapshot `opsOf` needs, so a param's first `when` line is proposed and applied alone — `apps/design-tool/app.ts:278`

`opsOf` prepends the param's own line only when `snapshots.get(issued)` has that param with `set === null`:

```276:279:apps/design-tool/app.ts
function opsOf(file: string, kind: string, target: string, when: string, value: string): Record<string, unknown>[] {
  const op = opOf(file, kind, target, when, value);
  const p = kind === 'set-param' && when ? snapshots.get(issued)?.params.get(target) : undefined;
  return p && p.set === null ? [{ op: 'set-param', name: lineName(target), when: [], value: p.default }, op] : [op];
}
```

`loadDesign` does `const number = ++issued` before it awaits `readDesign` (`app.ts:721`), and stores the snapshot only after that read, under `number` (`app.ts:793`). For the whole read, `snapshots.get(issued)` is missing. The previous snapshot is still in the map, under the old number. Answers interleave at their first await (`js/src/lib.rs:814`), and `design` is declared before `proposal` (`app.contract:572`, `app.contract:607`), so a commit that asks both runs `++issued` before `opsOf`.

`Card.fill` has no `params` line (`set: null`, default `accent.fill`). Open it on column `2-dark` and press "Only this mode". With the read settled, the proposal is two `set-param`s: `Card.fill = accent.fill`, then `Card.fill when dark-style=dim density=regular`. The server writes the line, then the `when`, and the inverse undoes them last-first (`tamber/src/edit.rs:416-450`), so one undo restores both. Redo replays `top.ops`, both of them. `lineName` spells both `Card.fill`; `param_names` keeps that spelling for the second op (`contract/cli/src/tamber_ops.rs:171-177`).

Now reload, or let a file change move `filesAt` (`app.contract:691`). `advance` also sets `proposeAt = changes.answer` on every poll (`app.ts:1044`), so the proposal is asked again in that same commit. `opsOf` sees no snapshot and sends only the `when` op. The server refuses it: `` `Card.fill` has no line in the design's `params` block: set its value before one of its `when` lines `` (`tamber/src/edit/params.rs:27-30`). Apply stays disabled because `proposal.ok` is false (`app.contract:1415`). The read finishing does not ask the proposal again — `proposal` does not take `design.answer` — so the error stays until the next poll bumps `proposeAt` (the poll waits up to 25s) or the user changes the draft. The same hole is there if they press "Only this mode" while `mutation written refreshes design` (`app.contract:640`) is still reading.

Apply of a proposal that was reviewed before the read is the other side of it. `reloadDesign` (`app.contract:957`) refreshes `design` and leaves the two-op diff on screen. Apply is enabled. `writeEdit` calls `opsOf` again (`app.ts:865`) and sends only the `when` op, which the server refuses. The diff and the write have come apart. A second Apply after the snapshot lands sends both ops.

Scrub uses the same check (`app.ts:903`). With the snapshot present, length is 2 and the scrub is not opened. With the read in flight and the old proposal still `ok` and Live, length is 1, so Scrub live opens a session for the `when` op alone and the server refuses that too.

## Smaller

### [P3] The scrub that is correctly refused says the change was never reviewed — `apps/design-tool/app.ts:911`

On a settled snapshot the two-op edit is Live, so Scrub live is offered (`app.contract:1418` checks the chip, not `opsOf`). Pressing it takes the `!op` branch and sets `scrubEnded` to "Review the change before scrubbing it.", shown at `app.contract:1425`. The review is on screen. The button should be the unavailable state the duration edit already uses.

## Round 1, on a settled read

These hold.

- An icon param no longer reaches `numberOf`. `paramRow` forces control `icon` (`components.ts:47`) and shows `sf star · material star` from `values[0]` (`components.ts:69-71`). `DialControl` has no field for that control (`app.contract:1626-1641`). `loadDesign` returns the panel.
- A panel edit sets `draftFor = ""`, `scopeOwn = false`, and `editWhen = ""` (`app.contract:731-736` and the same in `dialStepped`, `dialText`, `resetParam`). `draftStale` requires a branch draft (`app.contract:651`), so another param's slider does not disable Apply. A scrub of the same param cancels unless `editWhen == ""` (`app.contract:725`).
- A duration slider or stepper writes `` `${n}ms` `` (`app.contract:737`, `app.contract:751`). MapCard.pan starts at 300, bounds `0ms – 1000ms · step 50ms`. The free-text field (`control=token`, Card.fade) writes the typed string, so `200ms` is what the user types. Kinds with no dial control (shadow, type, easing) use that field (`components.ts:47`).
- The panel is built only for the app design (`app.ts:809`) and drawn only while `design.file == file` (`app.contract:1230`). The native mark uses hello's design file (`components.ts:90`), so MapCard stays `app.tamber#MapCard`.
- A token option matches by trimmed name (`dials.ts:134-135`). `#f00` does not select token `red`, and `brand.A` does not select `brand.a`. Dials call `controlOf` without `takesTokens` (`dials.ts:172`), so a colour dial still folds and a length dial still writes `Npx`. Clearing `scopeOwn` on a dial nudge means "Only this mode" after that nudge starts from `branch.value` again, which is what the token test already expects.
- The real-server setups that copy the stub design also copy `STUB_APP`.

Kept, as the round-1 note says: Card.fill's branch still says it follows `accent.fill` until Pin. That is the token editor, and pinning a literal there is the pinned param.

## Tests

Nothing covers the two-op path, so the hole above is invisible.

- `app.test.contract:117` types the duration slider to `400ms` and checks the icon text. The sentence "beside an open param's branch" then taps `cell-surface.card-0-light`, a token cell, so `branchOp` is `set-token` and `draftStale` cannot fire. The rung test at `app.test.contract:1122-1138` is the one that opens `Card.pad` and then edits MapCard.zoom. Neither presses "Only this mode", and neither looks at `editWhen` or the proposed ops.
- `app.test.contract:135` checks the Live chip, a scrub of Card.pad (it already has a line), and undo going from `(1)` to `(0)`. The stub's `transaction` returns an inverse and does not apply it, and `propose`'s diff is `ops[0]`, so a two-op proposal would still look like one line.
- `scripts/design-tool.test.mjs:342` does check bytes on the real server: Reset removes `Card.pad = 12px` and one undo restores the file; `Card.fill = #ff0000` is the short name; rescope to `accent.fill` undoes in one step. It never adds a `when` line.

The steady-state prepend, the short name, and the single inverse are right. The read window puts the round-1 refusal back on the path those tests never open.
