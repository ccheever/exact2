# Code review: tamber tool --via's dev loop probe tried again on a timeout, round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6e17fe650f`.
- **Method:** one brief (sha256 `79a91a343efb1d00f7ad1c90155b86a0fc8d8bee58db22236390fccce65f601c`), shared by both reviewers, each blind to the other's review; commit 6e17fe650f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (no findings).
- **Disposition:** none to take.

Nothing blocks. No findings.

The retry in `toolAnswers` (`scripts/tamber-server.mjs:1308`) matches the commit: a `TimeoutError` is tried again, up to `tries`, and every other outcome returns on the first attempt. Both `--via` call sites stay inside the approved budget, and the new glue test locks that behavior.

1. **Timeout only.** The loop retries only when `e?.name === 'TimeoutError'` and `i < tries`. On Bun 1.4.2 that is the rejection `AbortSignal.timeout` produces (`DOMException`, "The operation timed out."). A refused connection is a `TypeError` (`ConnectionRefused`) and is rethrown on attempt 1; `redirect: 'error'` is a `TypeError` (`UnexpectedRedirect`); an HTTP 500 fulfills `fetch` with `ok: false` and returns `{ app: null }` at `1309`. Concrete case: a tool is already serving, and `tamber tool --via http://127.0.0.1:9` takes the epoch check at `1353` (`design-tool.test.mjs:107`). Port 9 refuses in about a millisecond, `toolAnswers` returns null, and the command reports "not through" with the 5 tries unused.

2. **Loop.** The `for` at `1308` has the `try` as its body. `got` is assigned only when `fetch` fulfills, and a `Response` is truthy, so the loop stops. A `TimeoutError` before the last try is swallowed and `i` advances. On the last try, or on any other error, the `catch` rethrows into the outer `catch` at `1313`, which returns null. Five timeouts are five attempts and then null. One timeout and then a response keeps that response. A `TypeError` on attempt 1 or 2 stops there.

3. **Delay.** Each token attempt waits at most 2 s. Hello (`1311`, 10 s) runs only after a token response, so a target that never answers the token stops at 10 s. The two `toolAnswers(target, 5)` calls are mutually exclusive: the epoch check at `1353` sits inside `if (there)` and that branch returns, and the forward probe at `1367` runs only when `toolAnswers()` at `1345` got nothing. `1345` keeps the default `tries` of 1, so the check for a tool already at `TOOL_ORIGIN` stays one probe. On the epoch-check path a refused `--via` pays none of the 10 s; a slow loop pays up to 10 s of token tries, plus hello's existing 10 s only if a token attempt succeeds. That is the approved ~10 s budget, on both call sites.

4. **Tests.** In `scripts/tamber-tool-answers.test.mjs`, a first probe that sleeps 3 s loses to the 2 s abort, the second is immediate, and `asked()` is 2 (`24–28`). The default one try returns null (`30–31`) on a server whose second probe would have succeeded. A just-closed port with `tries` 5 finishes under 1 s (`35–37`); that bound is the "at once" requirement, and a refusal misread as `TimeoutError` would take about 10 s and fail it. A mute server with `tries` 3 is contacted exactly 3 times and takes at least 5.9 s (`38–41`), three 2 s aborts with 100 ms of slack. The file is on the glue list at `scripts/async.mjs:114` as `tamber-tool-answers`, which resolves to `./scripts/tamber-tool-answers.test.mjs`, beside the other tamber glue files, and outside `LEFT_OUT` and `EDITS_APPS`.
