# Code review: design-tool-harness's port, narrow verification (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort high, read-only sandbox, a throwaway worktree.
- **Method:** a narrow verification of the post-round BLOCKER fix 29ac151858, with the BLOCKER quoted verbatim; HEAD `29ac151858`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; the BLOCKER is closed.


The BLOCKER is closed: only the subprocess clears `EXACT_TOOL_PORT` and imports the harness, leaving the parent’s environment and module cache untouched. No new regression found.

The test still exercises HEAD~1’s fix: it caches 7740 first, then requires the harness’s port, origin, dist and published environment to match the checkout port. The old harness would fail.

The scenario is deterministic: fresh module cache, ordered imports, a path-derived port guaranteed distinct from 7740, and no socket or timing dependency.

Static review only; no tests, builds or edits.
