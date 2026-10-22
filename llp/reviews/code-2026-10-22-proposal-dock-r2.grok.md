# Code review: the design tool's proposal dock, round 2 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `4fb2fcb3bb`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; three SHOULD-FIXes, all taken post-round in 2a65e23709.

I'll review the proposal-dock follow-up as a read-only pass: the previous commit, HEAD, and whether the round-1 answers hold.The review is the follow-up commit against the previous one. I'll read both diffs and check the five round-1 answers in the code.The follow-up is three files. I'll read the dock, focus, and ready-line paths against the round-1 claims.Nothing blocks. The five round-1 answers hold for the paths they name, with two holes in the dock scroll and the ready line.

1. **SHOULD-FIX** `apps/design-tool/app.contract:1394` — `panelBackToPackage` docks and focuses `proposal-dock`, and never scrolls. Dial edits scroll `dial-<name>` to the top when `dockedFor` changes, and `resetParam` does the same on its press. Back to the package is the third docked press, and its button sits below the dial, so that row can remain under the dock.

2. **SHOULD-FIX** `apps/design-tool/app.contract:982` — `readyLine` stays up whenever `readyFor` equals the draft key. `markReady` (`1535`) sets that latch and nothing clears it (`cancelEdit` at `1419` does not). Changing a value and changing it back, or cancelling and retyping it, shows "Proposal ready: Apply or Cancel." while that proposal is still in flight and Apply is disabled. A re-read that settles `proposal.ok` false leaves the same line beside the error. A first-time key, and a re-read that stays applicable, behave as answered.

3. **SHOULD-FIX** `apps/design-tool/app.contract:1091` — the first `dialNumber` of an edit scrolls the dial to `block="start"` while a range drag is in progress (`2417` uses `input=onNumber`). The slider moves under the pointer and the next move retargets the value. A typed field keeps focus, and a later tick of the same dial does not scroll again.

Answers that check out: the delayed `focusApply` task is gone, and only those two presses call `focus("proposal-dock")` in the same commit as the dock. `dockedFor` is cleared on cancel and on `selectCell`, `backToBase`, `removeToken`, `setForApp`, `editState`, `editRow`, `moveRow`, `editFromSelection`, and `chooseFile`. The dock test pins the dock to `layout.viewport.h` and requires the field and the reset row wholly above it.
