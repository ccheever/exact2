# Code review: design-tool-harness's port, narrow verification (fable, substituting for grok)

- **Family:** Anthropic. Fable, read-only, in the round's throwaway worktree, with the same brief. It substitutes for Grok, which stopped twice without a verdict (the lead's 12:12 substitution rule).
- **Method:** a narrow verification of the post-round BLOCKER fix 29ac151858, with the BLOCKER quoted verbatim; HEAD `29ac151858`.
- **Verdict:** nothing blocks. One NIT, pre-existing and not introduced here: importing the harness stages the tool (stageTool), which a parallel run can do concurrently. It writes only content that differs.

1. Closed: the scenario runs in a child process with EXACT_TOOL_PORT deleted only in the child's copied environment; the parent's environment and module cache are never touched. Astra's SHOULD-FIX closes with it.
2. Nothing broken: the parent imports nothing that reads the port, and the harness's exports are unchanged for design-tool.test.mjs, design-tool-pair.test.mjs and design-tool-scale.test.mjs.
3. It still proves HEAD~1: against the old harness the child sees 7740, its origin and its dist, and the toEqual fails on three keys.
4. Deterministic: values derive from checkoutPort(ROOT), always 17741-17940, so never 7740; no port is bound and no turn taken.
