# Code review: a web stream's caller's abort, round 2, adversarial (sol)

- **Family:** OpenAI. GPT-6.1 Sol (`gpt-6.1-sol`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `1b02b5953f` (1 commit on `bbe444fb1d`).
- **Method:** one brief (sha256 `8793fa8bbafb1a6b063cb2f101aaad26fb20fbce10a9410b267a671b669ddb1e`), one reviewer, pointed at the commit as revised and at how the author settled round 1's findings, with fresh simplification and writing passes. The author (Claude, Opus 5.5) is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths, and its commit shas are of that worktree's history.

I found one blocking regression. No files were changed.

**Correctness finding**

**P1 — [host/web-js/ts-fetch.js:273](/private/tmp/claude-501/-Users-admin/8454b037-a354-4ecb-9fbf-e4c8a8685be5/scratchpad/sol/wt-stream/host/web-js/ts-fetch.js:273), used by `host/web-js/ts-stream.js:144`: the captured composer can still be app-controlled.**

`ts-data.js:8` imports the app before its `ts-fetch.js` import at line 11. An app can import a side-effect dependency before re-exporting its fetch-using answer. That dependency can obtain native `AbortSignal` through `new AbortController().signal.constructor` and replace `.any` without referencing any injected binding. `ts-fetch.js` then captures the replacement.

I reproduced this using the pinned Rolldown transformer and in-memory ESM modules. A replacement that attaches a stopping listener to each dependent signal produces:

| Initialization | Stream follows abort |
|---|---|
| Commit, app dependency first | No |
| Upstream main, same dependency | Yes |
| Commit, runtime first | Yes, with the original reason |

The new test replaces `.any` only after `beforeAll` loads the runtime, so it misses this ordering.

The shared helper also regresses file-fetch cancellation if the early dependency restores native `.any` after capture: cancellation waits for held module or file work. Upstream rejects before that work is released. This conflicts with the intended [Fetch abort behavior](https://fetch.spec.whatwg.org/#fetch-method).

**Proposed edit:** ensure `ts-fetch.js` evaluates before the app graph—for example, move its import before `__APP_TS__` in `ts-data.js`. Add coverage for a cold import with the preceding dependency and for cancellation during held file work.

I found no additional regression in ordinary fetch or `AbortSignal.any` validation. The existing argument-conversion test also passes in memory. Native dependent signals otherwise provide the intended event independence under the [DOM abort algorithm](https://dom.spec.whatwg.org/#abortsignal-signal-abort).

**Settlements**

| # | Result |
|---|---|
| 1 | **Incomplete.** Capturing the function fixes replacements made after runtime initialization, but the initialization-order failure above remains. |
| 2 | **Holds.** Non-null non-signals are checked inside `request()`’s error conversion, using Hermes’s message. `sequence()` retains its previous brand-check behavior through `isSignal`. |
| 3 | **Holds.** The saved listener is removed through `.finally()` on settlement. |
| 4 | **Holds.** `toBe` checks reason identity. |
| 5 | **Partly holds.** The specified rewrites are present; the new load-order explanation is factually incorrect, as detailed below. |
| 6 | **Holds.** Both named cap violations exist unchanged on `bbe444fb1d`. |

**Fresh simplification pass**

No additional finding. The shared composer and extracted brand check are appropriate; the required correction is initialization ordering.

**Fresh writing pass**

**`host/web-js/ts-fetch.js:270` and `1b02b5953f:message:11`** incorrectly say the app reaches `AbortSignal` only through these bindings and that no app code has run. The constructor access above disproves both claims.

**Proposed edit:** after fixing initialization, describe the explicit runtime-before-source import order and remove the “only through” assertion.

The Why paragraph is in the present tense, the stream comment qualifies composition support, and the test comments and names are clear. No other writing finding.

**Test Plan verification**

- **Focused test:** normal execution is blocked by sandbox `EPERM` at `stream-abort.test.mjs:12`. All four original test bodies pass against the source loaded in memory. Reverting each individual part fails its corresponding test.
- **Compiler:** `cargo build -p exact-web-js` is blocked by a denied temporary path.
- **Web JS suites:** attempted `abort-reason`, `answer-timers`, `held-count`, `scheduler`, and `upload`. Their claimed passes remain unverified because of the missing compiler build, missing Playwright, or denied temporary directories.
- **Web suites:** attempted all seven named suites. Temporary-directory and server-listen failures, plus missing Playwright, prevent verification of their claimed results and the comparison of 11 failures with upstream.
- **Caps:** confirms `navigation.js` at 1,503 counted lines and `foldables.md` at 3,254 bytes. Both files are unchanged from upstream. Its wasm checks additionally fail on sandbox-denied writes.
- `git diff --check` passes; the worktree remains clean.

Verdict: **not ready**.
