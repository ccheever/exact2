# Code review: the Figma plugin's leftovers after its review cap, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `44cf014020`.
- **Method:** one brief (sha256 `503e55b9fb06228c10901a7201c098c2e90bc56289bc3b642cae7329ae82374c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the range read-only: round-1 findings first, then the two new commits, without changing the worktree.The refusal count is the part I still need to reproduce: a collection already over the mode limit, and a pull that has to free every mode.Round 1's blocking finding is fixed: a parked mode dragged off the default is named, and the live mode is not. Undo still runs once per try that wrote, the new `undo()` clears `dirty`, and a refused pull does not remove a collection. `bun test tools/figma-tamber`: 61 passed, 330 assertions.

## Blocking

None.

## Should-fix

**`code.ts:226` — `wanted` is how many contexts still lack a mode, and the sentence uses it as how many modes to delete.** That matches only when the collection is already exactly at the limit. `addMode` also fails when the collection is already over the limit (a plan downgrade leaves the extra modes). Reproduced: limit 2, modes `dark-style=black, light` plus `P1` `P2` `P3`, one new context. The refusal says "needs 1 more place: remove 1 of `P1`, `P2`, `P3`". After one deletion, three modes remain and the next pull fails the same way. The number that frees a slot is `modes.length + wanted - limit` (here, 3).

**`code.ts:227` — when every mode in the way must go, and one of them is the default, the steps cannot be carried out.** `wanted` is right at the `addMode` failure (none of the new contexts matched a mode). Figma will not remove the last mode, and there is no mode the pull keeps that can be dragged above the default. The new test locks this sentence (`code.test.ts:854`): remove `Personal A` and `Personal B`, drag another above `Personal A`. Deleting the non-default leaves the default alone, and it still cannot be removed. The same clause at `code.ts:345` says "drag another mode above `Local review`" when that personal mode is the only mode in a collection a layout change would replace. Nothing else is there to drag. Both refusals are undone and the file is unchanged. The pull can proceed only if that collection is deleted.

## Nits

**`mapping.test.ts:3` — `plainWhy` is imported from `bun:test`, which does not export it.** The binding the test calls is the second import, from `./mapping`. `bun test` still loads the file. `bun -e` with that import throws `Export named 'plainWhy' not found`.

**`code.test.ts:760` — a renamed collection is now asserted; a variable or a collection already gone is not.** The check omits both (`code.ts:259` skips a missing collection, `code.ts:261` skips a missing variable). `holding` still reports them on `unread`.

## Checked, not filed

`plainWhy` matches the server sentence `tamber-under-light: the light side \`…\` is not the default's \`…\`` inside either wrapper (`the design would refuse it` and `…with the rest of the proposal`). Colour text from the checker has no backticks, so the two groups are the whole values. "Set it the same in every Light mode" is the right fix for the split layout, where each collection has a Light mode. The one-collection layout has a single `light` mode, and a push copies that value to every light context, so this refusal is not how that layout reports a light edit. Differing Light modes in the split layout are still asked about before the push.

Drops list each live variable in each split collection under its current collection name and variable name. A missing variable or collection is left out. No new path removes or overwrites a value on its own. The new refusal and drops assertions are exact strings and would fail on the parent commit.
