# Code review: tamber/1.16 inherited rows' fromAt and inspect select, round 2 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 2 of 3; HEAD `a0fd4a2ae4`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; Astra four SHOULD-FIXes, Grok one, all taken in c700c1d7a2.

No BLOCKER found. Four SHOULD-FIX findings remain:

1. **SHOULD-FIX — [scripts/tamber-builds.mjs:558](/tmp/rv-dt-inspwire/scripts/tamber-builds.mjs:558):** Select frames carry no plan or selection identity. After rebuild/reselection, a delayed frame can pass the new local guards and interpret old site numbers against the new plan. Bind the command to the plan and selection used to compute its sites.

2. **SHOULD-FIX — [scripts/tamber-inspect.mjs:651](/tmp/rv-dt-inspwire/scripts/tamber-inspect.mjs:651):** The fallback still fails when `App` only renders `Wrap`, which only renders `Label`: neither enclosing component owns nodes, so it returns `[]` and produces 409 instead of selecting the rendered root. It also stops at levels whose only nodes are siblings, which neither page can select as ancestors.

3. **SHOULD-FIX — [apps/design-tool/app.contract:1212](/tmp/rv-dt-inspwire/apps/design-tool/app.contract:1212):** `selectedUp` is consulted only for pending state; its returned error is never displayed. Server refusals such as 403/409 silently leave the selection unchanged. Render the mutation’s error beside the chain.

4. **SHOULD-FIX — [apps/design-tool/app.contract:942](/tmp/rv-dt-inspwire/apps/design-tool/app.contract:942):** `upExpect` never resets or correlates with the move’s response. After a successful move, independently clicking a shallower node triggers the “tree had fewer above it” note. Scope the note to the resulting selection and clear it on subsequent selections or failure.
