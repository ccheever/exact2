# Code review: the design tool's stage 13 E polish (the dry commit's key, the Files fold), round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `dd09005949`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; two SHOULD-FIXes (the colon guard, taken; a git-only HEAD move, not taken with reason) and a NIT (aria-expanded, taken).

Nothing blocks. The normal write paths, initial load, restart, and missed-answer handling refresh scope correctly. The fold hides only path/digest rows and preserves the design selector and section order.

1. **SHOULD-FIX — [apps/design-tool/app.ts:1211](/tmp/rv-dt-polish/apps/design-tool/app.ts:1211):** `endsWith(':')` rejects legitimate clients. Entering `Ada:` produces `design-tool:Ada:`, accepted by both the name form and server, but Commit remains disabled even after writes. Guard the exact unnamed sentinel `design-tool:` instead.

2. **SHOULD-FIX — [apps/design-tool/app.ts:1016](/tmp/rv-dt-polish/apps/design-tool/app.ts:1016):** Git-only HEAD changes never invalidate scope. For example, edit → undo disables Commit; an external empty commit then makes a fresh dry run return a HEAD-moved refusal, which should enable Commit to explain it. The cached state stays disabled. Include HEAD changes in invalidation or provide an explicit scope refresh. Click-time validation still protects enabled commits.

3. **NIT — [apps/design-tool/app.contract:2172](/tmp/rv-dt-polish/apps/design-tool/app.contract:2172):** Add `aria-expanded=filesShown`. The changing label describes the action, but the disclosure exposes no expanded/collapsed state to assistive technology.

No builds or tests run; nothing changed.
