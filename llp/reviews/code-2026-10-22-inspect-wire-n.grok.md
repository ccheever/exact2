# Code review: tamber/1.16 inherited rows' fromAt and inspect select, a narrow check of the round 3 fixes (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** a narrow check of the round 3 fixes; HEAD `618908d5a1`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one finding left (the refusal key by text), taken in 0ec98e07fd.

I'll verify those five Round 3 findings against `git diff b075ff6662 HEAD` only — read-only, no tree changes, no builds or tests.The workspace is a large tree. I'll locate the exact2 history and read only the two commits after `b075ff6662`.The diff is truncated in the middle. I'll read the rest and the surrounding logic those five findings depend on.Nothing blocks. Finding 1 is not fully closed. Findings 2, 4, and 5 are closed. Finding 3 is closed except for a comment omission.

**bug** — `apps/design-tool/app.contract:953`. A page refusal clears the move only when the Inspector's refusal text changes. `refusalsKey` is `id` plus `tail`, and `refusalRows` (`apps/design-tool/refusals.ts:25`) never includes `refusalSeq`, which `refuse` increments on every event (`refusals.ts:10`). A second press while that same text is still showing — Apple's fixed "cannot move its selection up", and the stub's — leaves `refusalsKey == upRefusalsAt`, so `upPageRefused` does not run and `upExpect` stays set. The next selection is still read as the landing. The new contract test covers the first change of that text only.

**nit** — `host/web-js/inspect-dev.js:297` and `host/web/dev.js:905`. Both comments still head the frame as `{up, levels, plan, from}`, and the line above each compare still says the selection and plan. `instance` is checked in the compare and named in the protocol clause (`docs/tamber-protocol.md:1426`).
