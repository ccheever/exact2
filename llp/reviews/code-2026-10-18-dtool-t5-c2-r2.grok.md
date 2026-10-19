# Code review: LLP 1122 T5 step 2, the tool's set-row editor, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `61bc295299`.
- **Method:** one brief (sha256 `fc6b755db6f8d8bb657abb5e43f6e049b8a4ea78072d91599679c7fd21a8270b`), shared by both reviewers, blind to the other review; commits 82d98133ff. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** taken in `54d7784b9f`: a held look's row that set-row writes has its editor (`heldrow-row-`), the token beside; the tests assert set-row's address as the tool sends it (`editTarget`, template and style). Not taken: a reload with the same build facts doesn't stale a draft (same plan, design, pin and files: the draft is still the same copy's).

Nothing blocks. Commit `82d98133ff` closes the round-1 design-tool points, and the copy-key change does not disturb the T3 or T4 editors.

## Propose-time file check

`proposeEdit` refuses `set-state`, `set-row`, `promote-param`, `promote-token`, and `detach-param` when the snapshot the proposal just read is not the one stored on the selection (`apps/design-tool/app.ts:832`). `selectionNow.files` is that digest, taken when the selection event is applied (`app.ts:1124`) and left alone by later `changed` events. The proposal reply's `files` is the pre-edit snapshot (`contract/cli/src/tamber_serve.rs:1204`).

Open `background-color` while the selection is still on digest F0 and the poll that would report another client's write of F1 has not been applied. Local `filesNow` is still F0, so `stateStale` is false (`app.contract:706`), but the proposal reads F1. The check returns "The files moved since this copy was selected" and never puts F1 on the review Apply would send (`app.ts:836`). A write that lands after a matching proposal is still the transaction's own conflict. T3 kinds (`set-token`, `set-param`, `set-dial`) are outside the regex.

## Heading

The row button still says "Edit this template · 2 instances" or "Edit style Panel". The editor heading is `editWords` (`inspector.ts:61`, passed at `app.contract:1324`, shown at `app.contract:1589`): `This template · 2 instances: background-color`, `Style Panel: color`, and, where the join set `state`, `This template · 1 instance: :active background-color`. The same `row` is what `set-row` writes (`inspector.ts:57`). With an authored `border-radius=8` shown as `border-top-left-radius`, the open editor names `border-radius`.

## Copy key

`inspected.key` is now build, node, site, instance, and `s.at` (`inspector.ts:95`). `s.at` is `buildKey`: revision, scheme, mode, plan, design, pin (`inspector.ts:33`). Pin the build, then click the same node again. The new selection clears `inspected.stale`, but the key no longer matches `stateFor`, so Apply stays off until Edit is clicked again (`app.contract:867`, `:884`, `:902`). A re-report with the same build facts does not do that.

That key is only compared from `stateStale` and those three open actions. T3's branch draft is `draftFor` / `branchKey` / `partsKey` (`app.contract:708-729`). A pin does not retarget a token or param edit through this change. T4 `set-state` shares `stateStale`, which is the pin behavior round 1 asked for.

## Undo words

A style `set-row` (and a style promote, same helper) is named `Panel color (styles.contract) = blue` (`moves.ts:33-38`). The address already carries `file` (`inspector.ts:57`). Two files that both declare `style Panel` no longer read the same in "Can't undo …". A template row stays `background-color at card.contract:3`.

## Not taken

Leaving the outgoing op unasserted is fair. `set-row` spreads the join address and only then sets `value` (`app.ts:241`); the join tests own that address. The contract grammar does have `\"` (`docs/contract-grammar.md:52`), so "no escaped quotes" is not the real constraint. A whole-text compare of the stub diff would still be a poor lock.

Skipping a real compile inside the stub is also fair. `app.test.contract` drives canned replies. The fixture change is the part that matters here: the style row is `color` / `red` in `styles.contract`, not `padding` in `card.contract` (`app.test.contract:1412`, heading at `:1421` and `:1440`).

The new refusal and the pin-reselect path are not what that test drives. It still uses the stub's `sha256: "stale"` sentinel and a `changed` event that moves `filesNow`. The stub's propose `files` are the same map as the poll, so the new check passes whenever Apply is reached.
