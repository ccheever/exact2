# Code review: tamber/1.16 inherited rows' fromAt and inspect select, a narrow check of the round 3 fixes (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** a narrow check of the round 3 fixes; HEAD `618908d5a1`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one finding left (the refusal key by text), taken in 0ec98e07fd.

No BLOCKER, but #1 is only partly closed.

- **P2 — [app.contract:953](/tmp/rv-dt-inspwire/apps/design-tool/app.contract:953):** `refusalsKey` tracks displayed text, not refusal events. Retrying a move that receives the same refusal leaves the key unchanged and `upExpect` armed. Refusals beyond the first three are also invisible to this key ([refusals.ts:25](/tmp/rv-dt-inspwire/apps/design-tool/refusals.ts:25)). Track the selected build’s refusal counter instead.
- **NIT — [inspect-dev.js:297](/tmp/rv-dt-inspwire/host/web-js/inspect-dev.js:297), [dev.js:905](/tmp/rv-dt-inspwire/host/web/dev.js:905):** both frame comments still omit `instance`; the protocol clause includes it.

#2, #4 and #5 appear closed by inspection. No other new problem found. No changes, builds or tests performed.
