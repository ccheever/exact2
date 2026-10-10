# Code review round 3: exactBodyFrom (9dca6be37), grok

Reviewer: Grok 4.7 (xhigh, read-only), 2026-10-08, a confirming round run by the lead on 9dca6be37.

I'll review the `exactBodyFrom` change read-only, starting with AGENTS.md and the two folded review rounds, then check each round-2 finding on every executor.Round 2 named six findings. I'll read the fold and the current executors to see whether each one is actually closed.The confirming pass is still open. I'll re-check each round-2 fix on the current tree before writing the landing call.Nothing blocks landing. Round 2's six findings are fixed on the executors that send a body. No remaining path sends another file's bytes, drops a non-empty upload, hangs the caller past the accepted native read, or reads outside `fs.read`.

**Round 2**

1. **Old shells.** Hermes cohorts put `typescriptRuntime: 1` in the compatibility inputs (`bake/src/compat.rs:393`) and copy it onto `app.plan` requires (`bake/src/receipt.rs:508`). The classifier treats a missing key as 0 (`scripts/app.mjs:1361`), so an already-installed cohort is `bundle: false`. `publishStream` checks that again (`scripts/deploy.mjs:1104`) and the publish loop skips `binary` rows (`scripts/deploy.mjs:1359`). The new key also changes the compatibility id, which is what those shells poll. Web is store level 0, so the wasm page and the JS target ship with the page, not through that updater.

2. **Deadline before send, native.** The deadline is armed before `body::resolve` and checked before `fetch_request` (`host/apple/src/executor_core.rs:924`). A late read returns Timeout and does not send. The stream opener checks the abort signal before `fetch_request` (`host/apple/src/executor_stream.rs:165`), so an abort during the read does not send. The read itself still runs to completion and holds the worker (`host/apple/src/executor_body.rs:15`). That is the same stall as `readFile`, and the fold left it that way. iOS, macOS, and Linux share this executor.

3. **Web clock.** Both carriers measure the file read and return before `fetch` if the budget is already spent: `host/web-js/ts-fetch.js:55` throws before `Math.max(1, …)`, and `host/web/http-body.js:203` returns kind 10. A blocked event loop no longer gets another millisecond and sends.

4. **Already-aborted web read.** `host/web/http-body.js:195` returns before `bodyFile` when the signal or the deadline is already aborted, including a controller aborted while `httpHelpers()` is still loading (`host/web/glue.js:743`). The JS target rejects inside the race (`host/web-js/ts-fetch.js:46`), so the caller settles and `fetchWith` is not reached.

5. **JS-target stream `Request`.** `host/web-js/ts-stream.js:23` uses `request.url` and `request.headers`, with `init.headers` overriding. A POST `Request` no longer becomes `"[object Request]"` and drop its headers.

6. **UTF-8 cap.** The prelude counts UTF-8 bytes (`js/src/prelude.js:654`, checked at `:801`). The JS target and `http-body.js:190` use `TextEncoder`. Rust uses `path.len()` (`runner/src/request.rs:337`). All four reject a path longer than 4096 bytes, including the `app:/` prefix. A lone surrogate fails closed at JSON parse.

**Also checked.** Windows hits `#[cfg(not(unix))]` after `stat` and returns Unsupported without a body read (`host/apple/src/executor_body.rs:122`). The Rust logic ABI refuses the field instead of encoding it (`logic/abi/src/lib.rs:213`). Grants are narrowed with `storage::scope` / `scopedGrantSet` and cannot widen; `coversPath` runs before the storage import; unix reads go through pinned directory fds and `O_NOFOLLOW`. WebSocket plus `exactBodyFrom` is refused before the socket on the prelude, in `body_from_refusal`, and at `host/web/http-body.js:169`. The wasm page reads through `storage-request.js` `requestBody` on the page store, not the SQLite worker. An empty file matches an empty byte body: native omits it (`host/apple/src/executor_core.rs:1024`); the page sends an empty blob.

**Nit.** `host/web-js/ts-fetch.js:21` — a stream whose input is a `Request` to `wss:` does not TypeError in `bodyFromRefusal`, because `String(input)` is `"[object Request]"`. `http-body.js:169` still refuses before `readSocket` and before the file read, so nothing is sent. Test `request?.url` there too.

**Nit.** `host/web-js/ts-fetch.js:51` — a signal that is already aborted rejects the caller, but `readBodyFile` is still started. Check `signal.aborted` before that call, as `http-body.js:196` does.

**Nit.** `scripts/deploy.test.mjs:345` stubs `requires.typescriptRuntime`. The compat-input test does not notice if `receipt.rs` stops copying the key. Assert the baked `app.plan` requires instead.

Nothing blocks landing.
