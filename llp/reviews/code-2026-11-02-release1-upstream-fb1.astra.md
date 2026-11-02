# Code review: release1's export patches upstreamed (optional Snapback, inlined helper, docs corpus) (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `dac072c79a`.
- **Method:** one brief (sha256 `260e2084d2c2fe86b9dd8377ce05a2d8b5f10079e6e571459ccb2afc8974724b`); a quick review, the design approved by Charlie. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.

No BLOCKER, MAJOR, or MINOR findings.

- Snapback-present paths retain the same calls and arguments. No reachable null dereference found: [exact.mjs:1333](/Users/admin/x2scratch/fb1-rv-reds/scripts/exact.mjs:1333) follows `wireSnapback`’s throwing guard; agent-test calls use optional chaining.
- The inlined projection helper is byte-for-byte identical to `snapback4/online/projection.ts`.
- Corpus tests remain meaningful: 71 titled documents retain nonempty-block, heading, and table assertions; 75 documents exercise Markdown invariants. Seeded parser comparisons and resolver-call assertions remain intact. `card.md` still participates in the invariant test.

Read-only source review; builds and tests were not run. Apple’s Git launcher attempted cache writes, which the sandbox blocked; subsequent Git reads used the direct binary.

VERDICT: APPROVE
