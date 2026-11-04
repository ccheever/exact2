# Code review: the web test scaffolds, the whole branch, round 2, adversarial (sol)

- **Family:** OpenAI. GPT-6.1 Sol (`gpt-6.1-sol`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `1795f13e18` (2 commits on `bbe444fb1d`).
- **Method:** one brief (sha256 `71084faf335951dd5cd6ab6f3c7d0690032cf56fb6da7cadad3f557682bf7c50`), one reviewer, pointed at the commits as revised and at how the author settled round 1's findings, with fresh simplification and writing passes. The author (Claude, Opus 5.5) is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths, and its commit shas are of that worktree's history.

All four settlements hold. I found no defect in the changed code, but one factual correction remains in the first commit message.

**Settlements**

1. **Navigation root: fixed.** Both nested `main` elements explicitly carry `navigationBack=""`. The compiler recognizes that combination as a navigation root, and the emitter enables `navigationRoots`. Each `main` still contains only the loop, preserving the whole-parent replacement case. The cited document test does cover visibility, coverage and inertness beneath sheets; duplicating those assertions here is unnecessary.
2. **Exit race: fixed.** `press` clicks, observes removal or `data-exiting`, and pauses any exit animation within one browser evaluation. Both tests use it. Subsequent browser calls cannot consume the remaining 400 ms exit.
3. **First message’s tense: fixed.** The explanation uses the present tense.
4. **Second message’s writing: fixed.** It states the route requirement positively and removes the unchanged-steps sentence.

**Fresh correctness pass**

No findings in either commit’s changed code or the two queue-entry deletions.

**Fresh simplification pass**

No findings in either commit.

**Fresh writing pass**

- **`1122a34696`, commit message:6:** “`rt.js` imports `../web/timer-observations.js`” incorrectly identifies a direct import. That specifier belongs to [host/web-js/timer-observations.js:1](/private/tmp/claude-501/-Users-admin/8454b037-a354-4ecb-9fbf-e4c8a8685be5/scratchpad/sol/wt-scaffolds/host/web-js/timer-observations.js:1), reached through `timer-waits.js`. This directs a reader inspecting the dependency to the wrong module and falls short of `AGENTS.md`’s clarity requirement. Replace the sentence with: “The runtime’s module graph requests `/web/timer-observations.js` through the re-export in `host/web-js/timer-observations.js`.”

No other writing findings in the changed code, comments or either message.

**Verification**

Both requested commands were attempted from `host/web-js`:

| Command | Observed result |
|---|---|
| `bun test shared-reorder.test.mjs` | Stops before either case: missing `playwright-core`. |
| `bun test sheet-leave.test.mjs` | Both cases stop before compilation: sandbox `EPERM` creating `target`. |

For the remaining Test Plan claims:

- **Reorder failure on upstream main:** a read-only execution of the URL-resolution expressions confirms that the old resolver selects nonexistent `host/web/web/timer-observations.js`; the new resolver selects the existing intended file. Browser behavior remains unverified.
- **Sheet compilation failure on upstream main:** the baseline fixtures lack `navigationKey`, and the compiler’s `lower-route-place` check rejects their computed `navigationPresentation`. This is confirmed from compiler code, without executing compilation.
- **Passing repeat runs:** neither commit’s claimed passing runs or repeat counts could be independently reproduced.
- **`sheetLeaves` returning `false`:** code inspection predicts the first test fails in normal-motion mode because `press` returns `null`, where `true` is required.
- **Decorated-textarea guard removed:** code inspection predicts the second test fails because it observes and pauses an exit, returning `true`, where `null` is required. Neither mutation was executed.

`git diff --check` passed. No files changed.

Verdict: not ready.
