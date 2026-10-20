# Code review: the Figma plugin's leftovers after its review cap, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `ddf24a4cc8`.
- **Method:** one brief (sha256 `d60ef9cfbdd6b3f47584aa1b8bdc2a4e821ec1cf3f94fce0c64d71a90bc69939`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review HEAD (`ddf24a4cc8`) read-only: the refusal naming, undo count, and drops list, then run the plugin tests.The commit touches the pull check, room refusal, and UI. I'll read the diff against the surrounding plugin code and the round-9 notes.The diff is small. I'll read the pull, undo, and refusal paths in full so the new names and undo timing can be checked against the tests.The refusal is built after undo and looks collections up by id. I'll trace that against the split retry, a new collection, and the tests.The refusal reads the default mode after undo. I'll reproduce the case where that mode is a live context and a parked mode holds the slot, and run the plugin tests.Tests passed. Next I’ll reproduce a pull where the parked mode is no longer the default, and check the refusal text.The room refusal names the wrong mode when a parked mode is not the default. Undo, `dirty`, and the drops list behave. `bun test tools/figma-tamber`: 59 passed, 322 `expect()` calls.

A pull that never writes undoes nothing, and each try that does write is undone once. The new `undo()` at `code.ts:327` clears `dirty`, so the catch at `code.ts:358` does not undo again and does not touch the person's last edit. Nothing is left half-written: a refused pull throws before `old.remove()`, and the split collections from that step are undone. No path removes or overwrites a value it did not already. `collectionId: null` does not crash (`col?.` and `col &&`); a collection created in that try is not a case this planner hits, because `pull` at the file's limit never asks a new collection for more modes than the limit. Personal mode ids on a collection that was already in the file are still there after undo.

Drops list each live variable in each split collection under its current name and the collection's current name. A variable or a collection already gone is omitted; `unread` names it. Reproduced: delete `Tamber · dark-style=black` and dim's `accent/fill`, and the only drop is dim's `accent/soft`.

## Blocking

**`code.ts:226` names the live default as the unused mode.** With no personal mode, the sentence uses `col.defaultModeId` and says Tamber keeps that mode unused, then tells the person to remove the collection. A parked mode stays parked after it is dragged off the default. Reproduced: collection `Brand`, modes `dark-style=black, dark` (default, the context this pull keeps) and `Dimmer` (the parked mode). The refusal says `` `Brand`'s default mode `dark-style=black, dark`, which Tamber keeps unused ``. Deleting `Dimmer` alone lets the pull succeed. The refusal is what points at the live mode and at deleting the collection.

## Should-fix

**`code.ts:224` always says "remove one".** If every mode left is one the person added, each new context needs a free slot. Reproduced at a limit of 2 with `Personal A` and `Personal B` and both of Tamber's modes already gone: the refusal names both and says remove one. After `Personal B` is removed, the pull is refused again for `Personal A`.

**`code.ts:347` tells the person to delete a mode that may be the default.** The room refusal says to drag another mode above a default first. This sentence does not. `Personal A` as the default cannot be removed until another mode is dragged above it, so the instruction stops short of a pull that can proceed.

## Nits

**`ui.ts:82` omits the token.** The check payload carries `token`, and the dialog shows the collection and the live variable name. `brand/primary` is not tied back to `accent.fill`.

**`code.test.ts:788` only renames `modes[0]` while that mode is still the parked default.** It locks the `Brand`/`Dimmer` sentence and would stay green for the wrong-mode case above. The new message and drops assertions would fail on the previous commit. The undo counts match the old control flow, so those numbers alone would not.
