# Code review: tamber tool --via's canvas check (a page that won't let the tool frame it), round 2 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `9936d5baa9`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one NIT (the advice from stripped names), taken in 12d92fdfa7.

I'll review the round-2 diff against the round-1 answers, read-only, starting with the two commits and the review instructions.The tree is clean at `9936d5baa9`. I'll read both commits and the frame-ancestor matching code against the round-1 answers.The round-2 diff is four files. I'll read the matcher, the wording, and the call sites that still depend on the old behavior.Nothing blocks. The seven round-1 answers are in the code, and they hold for the dev loop's real header (`frame-ancestors 'self' http://127.0.0.1:<port>`).

1. **NIT** `scripts/tamber-frame.mjs:80` (same check at `apps/design-tool/canvas-check.ts:15`). The port advice is a string match on the display name, after the path has already been stripped. `http://127.0.0.1:7740/__tamber/tool/` is shown as `http://127.0.0.1:7740`, so both surfaces say to set `EXACT_TOOL_PORT`, but that path never matches an ancestor. A schemeless `127.0.0.1:7740`, which `matches()` does accept, is shown without `http://`, so the advice says the policy cannot be fixed by a port when `EXACT_TOOL_PORT=7740` would make it match. The loop's own header is `http://127.0.0.1:<port>` with no path, and that case is right.
