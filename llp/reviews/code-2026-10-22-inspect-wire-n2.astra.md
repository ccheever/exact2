# Code review: tamber/1.16 inherited rows' fromAt and inspect select, a second narrow check (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** a second narrow check; HEAD `0ec98e07fd`. The author (Claude) is not a reviewer.
- **Verdict:** Astra: one finding left (a global count), taken in eb6c933479; Grok: closed.

Not fully closed: one **P2** remains.

- **[refusals.ts:30](/tmp/rv-dt-inspwire/apps/design-tool/refusals.ts:30)** — The summed counter is global. While build A’s move is pending, a refusal, refusal-clear, or removal of build B changes it, causing [app.contract:956](/tmp/rv-dt-inspwire/apps/design-tool/app.contract:956) to drop A’s `upExpect` prematurely. A subsequent higher-than-requested landing then loses its explanatory note. Track refusal events for the selected build.

Repeated wording and the three-row limit are addressed; both frame comments include `instance`. Reviewed HEAD `0ec98e07fd` read only; no builds or tests run.
