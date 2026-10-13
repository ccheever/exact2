# Code review: LLP 1120 §5's measurement counts and connection log, 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `554ad2111f094f6c4d6108327c56d98a7d14e7b579a402119e26e320b6f57f91`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Every finding taken in the second commit; Grok's two NITs in the landing commit.

---

## MINOR

**`vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:232`** — Session endpoints are formatted with `"%@:%@"`. The lane path uses `endpoint()` in `darwin_task.mm`, which writes `[fe80::1]:443` and, when the port is missing, the address alone. The session line writes `fe80::1:443`, and a nil port becomes the characters `(null)` (`%@` of nil). Equality still groups connections inside the session arm. A parser written for the lane form will split one IPv6 connection, or fail to match the two arms.

**`vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:21`** — The first `open` is stored in a `OnceLock`, and failure becomes `None` through `.ok()?`, the same state as an unset variable. A missing directory, a typo, or a permissions error turns the log off for the rest of the process and prints nothing. `enabled()` then stays false, so the session stops preparing records too. A later correction of the path is not retried.

## NIT

**`vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:17`** — `OpenOptions::create(true)` uses the umask, typically mode `0644`. The line holds the full URL and both endpoints. The file is created only when `IBEX2_HTTP_METRICS` is set. Mode `0o600` would match that.

**`vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:60`** — `write_all`'s error is ignored. The mutex stops concurrent callbacks from interleaving a line. A short write that then fails still leaves a partial line, and the next record is appended after it.

## Checks that hold

Unset, nothing moves. `option_env!` of an absent `EXACT_HTTP_WORKERS` or `EXACT_HTTP_IN_FLIGHT` returns the previous literals: 6 independent workers (`executor_core.rs:17`, plus the ordered owner) and in-flight cap 32 (`executor_async.rs:40`). That cap is also the independent lane's per-host connection limit, which this constant already was. `PER_ORIGIN` stays 6. Rustc records both names as `# env-dep:` in the crate dep-info, so a changed value rebuilds. With `IBEX2_HTTP_METRICS` unset, `file()` reads the environment once, caches `None`, and does no file I/O. The session callback returns at `ibex2_metrics_enabled()` before it builds endpoint strings. The lane's `note()` returns before it builds JSON.

The const parser fails closed. Empty, a sign, whitespace, and any non-digit trip `assert!`. `0` fails `n > 0`. Overflow uses `checked_mul` and `checked_add` and panics with "a build count past usize", so a const initializer fails the build instead of wrapping. The new test covers `None`, `"7"`, `"32"`, `""`, `"0"`, `"7a"`, and a value past `u64::MAX`. A7 is `EXACT_HTTP_WORKERS=7`. C7 is `EXACT_HTTP_IN_FLIGHT=7`. Defaults are A6 and C.

JSON escaping is `serde_json`: quotes, backslashes, and controls are JSON escapes, and absent fields are `null`. Same-process appends are one `Mutex<File>` opened once. `write_all` runs while that lock is held, so callbacks cannot tear a successful line. Synchronous append on the delegate queue, when the log is on, is acceptable for this build. The exchange condition is released first. The write is one line, on both arms, and is small next to an LTE round trip. It does not take the state lock or the finisher lock, so it does not deadlock the lane. `queued_ms` is null on the session arm because that exchange does not store a resume time. The lane records the resume-to-fetch-start gap. That is the async queue measurement. The session arm has no such queue.

Nil metrics, an empty transaction list, and nil addresses take the `last == nil` path: reused `-1`, null protocol and endpoints, no crash. A task the exchange has already dropped is still the callback's `task` argument for the duration of the call. `originalRequest.URL.absoluteString` lives until that call returns under ARC, and the Rust side copies the bytes before returning. A nil URL becomes an empty `url` string. URLs and endpoints are written only when `IBEX2_HTTP_METRICS` names a file that opens. An exchange is logged once, as `session` or `lane`, on whichever transport ran it.

## READY
