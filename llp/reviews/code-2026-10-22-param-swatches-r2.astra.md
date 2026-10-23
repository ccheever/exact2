# Code review: the design tool's colour param swatches and chain words, round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `6009688035` (shas before the rebase onto main). The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; two SHOULD-FIXes (system colours, visible labels), taken post-round.

1. **SHOULD-FIX — [components.ts:192](/tmp/rv-dt-swatch/apps/design-tool/components.ts:192):** Direct or app-set system colours such as `Canvas` survive the filter unchanged. Both Light and Dark swatches then inherit the tool’s scheme, so they paint identically instead of representing their columns. Use per-side fallbacks, or omit unresolved system-colour swatches as for `-exact-` roles.

2. **SHOULD-FIX — [app.contract:2477](/tmp/rv-dt-swatch/apps/design-tool/app.contract:2477):** Each swatch’s label and value exist only in `aria-label` and `title`. The visible UI contains unlabelled squares and the chain text, so “labelled, value as text” remains unmet. Render each column label and value beside its swatch.

Nothing blocks. The other listed round-one fixes look correct by inspection. No builds or tests run.
