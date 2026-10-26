# Code review: tamber/1.16 inherited rows' fromAt and inspect select, a second narrow check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** a second narrow check; HEAD `0ec98e07fd`. The author (Claude) is not a reviewer.
- **Verdict:** Astra: one finding left (a global count), taken in eb6c933479; Grok: closed.

I'll verify the refusal-count fix against HEAD only: read the code and the contract test, and leave the tree untouched.The diff matches the claimed shape. Next I’ll trace `refusalSeq` into `refusalsKey` and check the contract test actually covers a repeated refusal.Nothing blocks. The earlier refusal-key and `instance` comment issues are closed.

`refuse` increments `refusalSeq` on every refusal, including the same words again. `refusalRows` sums that counter across every build, so a fourth build is included. `refusalsKey` is that sum, and `selectUp` snapshots it before the move. A later refusal changes the key and `dropUp` clears `upExpect`. The repeated-refusal contract check requires that second increment. Both frame comments name `instance`.
