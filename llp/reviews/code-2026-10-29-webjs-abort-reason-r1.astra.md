# Code review: web-js, an aborted fetch rejects with its signal's reason, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f4876d0a93`.
- **Method:** one brief (sha256 `43478193b904bd57c0dd7de95f1621fb65b2d8a84ea9cf094386b2ce7af2eea9`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Existing assertions contradict the new behavior.** [host/web/request-refusal.test.mjs:1495](host/web/request-refusal.test.mjs:1495) expects `['FetchError', 'Aborted']`; line 1497 also expects `kind === 'Aborted'`. Both now receive the Request signal’s `AbortError` reason, so this existing test fails. Update both assertions and the preceding comment to check reason identity and `name === 'AbortError'`.

2. **The arrival poll can hang after a valid timeout.** [host/web-js/abort-reason.test.mjs:38](host/web-js/abort-reason.test.mjs:38) waits exclusively for server arrival, but the three 30 ms timeout cases can reject before arrival on a loaded machine. The loop then ignores the settled promise and never reaches `finally`. Await rejection directly for timeout cases; for explicit aborts, use an arrival notification raced against premature settlement and a bounded watchdog.

**Non-blocking**

1. **Browser coverage could be stronger.** [host/web-js/abort-reason.test.mjs:42](host/web-js/abort-reason.test.mjs:42) exercises Bun’s fetch, with every request held before headers. Extend the existing browser fixture to check reason identity, `signal: null`, cancellation during body reading, and both deadline/own-signal race orders. The new native test is deterministic and checks identity through `holding`.

2. **Classification changes intentionally.** [host/web-js/shape.js:55](host/web-js/shape.js:55) classifies an uncaught `TimeoutError` as `error`, replacing the previous `offline`; this matches native. Bare aborts remain `error`, and `exactTimeout` remains `timeout`. No affected app/package dependency or caller/journal assumption requiring the old abort kind was found.

The runtime change otherwise looks correct: `AbortSignal.any` retains the first abort reason, so a later own-signal abort cannot overwrite a winning deadline. Request signal inheritance and explicit `null` remain intact, and ordinary body-read aborts reach the new catch branch.

Doc issues: none found. The reference already promises signal reasons; its and agent-pitfalls’ remaining `FetchError('Aborted')` statements concern native superseded sends.

Read-only inspection; no network, tests, builds, or edits.
