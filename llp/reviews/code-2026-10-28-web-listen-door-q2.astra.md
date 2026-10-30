# Code review: the web realms' load-time listen door (x2/web-listen-door), narrow re-check, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `2e98d808af`.
- **Method:** one brief (sha256 `dcbecbcae4769cba4395160f55be88fba2e4ed877558b7dfd1fa5359cd50b825`), one reviewer; a quick review of a mechanical main-red fix (the lead's 15:41 ruling: a quick review plus a narrow re-check). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** One P3 (the test's wording overstated which calls are refused): the test and its commit now say only the listen ask is newly let through.

1. **P3 — The new test overstates “every other call … refused.”** [module-storage.test.mjs:250](host/web/module-storage.test.mjs:250) checks callback, store-read and request refusals. The [page door](host/web/module-glue.js:129) also accepts op **13** for journaling, and both it and the [worker door](host/web/module-worker.js:42) return `undefined` for op **21** outside an answer. These exceptions predate this diff. Only `(12, 'listen')` is **newly** allowed; qualify the test’s wording accordingly.

Otherwise clean:

- The fake-clock stub is installed before both `binary.js` and `prelude.js`, correctly answering the load-time probe with `''`.
- The setter captures the original production function without wrapping it, then installs a writable, configurable value. The prelude’s read and deletion still work. Enumerability differs, but nothing in this path observes it. Assertions run after the answer clears its context.
- Both production additions strictly check `op === 12 && name === 'listen'`; previously refused calls remain refused.
- I found no remaining full-prelude evaluation without a suitable door in the tracked repository.

Static review only; no tests or builds run.
