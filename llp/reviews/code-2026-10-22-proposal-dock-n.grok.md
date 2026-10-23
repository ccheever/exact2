# Code review: the design tool's proposal dock, a narrow verification of the post-round fixes (BLOCKER quoted verbatim) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** a narrow verification of the post-round fixes (BLOCKER quoted verbatim); HEAD `2a65e23709`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; every round-2 finding closed.

I'll verify the round-2 fixes against HEAD only: read the commit and the dial/dock paths, and change nothing.The contract diff matches the claimed fixes. I'll read the surrounding actions to see whether they close the findings cleanly.Nothing blocks. The six round-2 fixes are in HEAD and they close those findings.

1. A successful Apply or Keep clears `dockedFor` and `readyFor` with the written draft (`app.contract:1461`). A later step or text edit scrolls because the action reads the dock from before the write, so `not docked` is true when that draft’s dock is already gone (`app.contract:1110`, `1138`).
2. Both param rows call `panelBackToPackage(r.packageToken, r.c.name)` (`app.contract:1996`, `2002`). That scrolls `dial-${row}`, which is the id on that param’s control.
3. `panelBackToPackage` no longer assigns `file`. The component list is filled only for the app design, so the proposal stays mounted and `focus("proposal-dock")` still has its target.
4. `cellText`, `partEdit`, and `onlyThisMode` set `dockedFor = ""` (`app.contract:1283`, `1299`, `1317`), so a branch edit does not keep a panel dock.
5. The ready line is `readyFor` matched and `applyAnswer` (`app.contract:982`). `applyAnswer` is the in-hand answer for this draft, ignoring in-flight reads and writes (`app.contract:987`). A failed re-read or a value whose answer is not this draft says nothing. Cancel and `afterWrite` clear `readyFor`.
6. `dialNumber` does not scroll (`app.contract:1084`). `dialReleased` scrolls only when this dial’s dock is already open (`app.contract:1121`), which is after the drag’s updates have committed.

`applyReady` is the same condition as before: `applyAnswer` plus no proposal read or write in flight.
