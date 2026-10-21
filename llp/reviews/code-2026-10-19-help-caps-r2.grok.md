# Code review: caps runs the helps' flag parity before every push (scripts/help-parity.mjs), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ed11a1803c`.
- **Method:** one brief (sha256 `1ff9a97f027b470b4a65cf729bbf000e495fc78c9d580dfb11fe48c408e26b8f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking
- **Disposition:** none needed

Nothing blocking.

The round-1 fixes hold, including the three risks called out:

1. **Slices fail closed.** `slice` throws if the start marker is missing, or if the end marker is missing after it. On the markers that exist today the window is the same as before, so a present file still parses the same flags.
2. **A source error does not skip the other checks.** `flags` stays `[]`, `help:sources` is recorded, and the length, hidden-row, index, and form checks still run. A missing `web`, `test`, or `agent` entry is reported as `help:verb` instead of throwing on `helpText(...).length`.
3. **The throwaway caps repo does not pick up extra violations.** Nothing is `git add`ed, so `git ls-files` is empty and the line, recipe, and harness checks do not run. The rules fixture is under its word, check, and document caps and has no product-latency placeholder, so the two stub findings are the only ones and the exit line is `2 violations.` Each `caps()` call is a new process with its own absolute `require` path, so the module cache does not carry a stub or a throw into the next run. No `scripts/help-parity.mjs` returns before the helps line.
4. **The hidden-row check matches `help.test`.** It splits a row on ` · ` and looks for the first word of each spec in `--help all`, and only for `web`, `test`, and `agent`. Wording and form assertions stay in `help.test`.
