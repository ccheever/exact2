# Code review: LLP 1120 §5's measurement counts and connection log round 2, 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `0f121b4fc65a1717de0d011d30044e2cf4f66e963d3a60d2568d39273060e328`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The two NITs (mode 0600, an open failure said once) taken in the landing commit.

---

## MINOR

**Session endpoints are not spelled like the lane's** — `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:232`

The lane formats an address through `endpoint()` in `darwin_task.mm` (`[fe80::1]:443`, and the address alone when the port is nil). The session log formats with `"%@:%@"`, so the same pair is `fe80::1:443`, and a nil port becomes `addr:(null)`. Within one arm the full string still identifies a connection. Across A7 and C7 the same IPv6 connection is two different strings, and a colon-split that assumes the lane's brackets mis-parses the session arm. Use the same formatter on both.

## NIT

**The log file is created with the process umask** — `vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:17`

`OpenOptions` has no mode, so a typical umask creates the file as `0644`. The lines contain full URLs. `OpenOptionsExt::mode(0o600)` was the round-1 nit and is still absent.

**A failed open is cached as "no log" and never reported** — `vendor/ibex/crates/ibex2/src/transport/metrics_log.rs:21`

`.ok()?` inside `OnceLock::get_or_init` stores `None` for the process. A bad path, a missing directory, or a permission error then looks the same as `IBEX2_HTTP_METRICS` being unset: no file, no message on stderr. That was the other round-1 nit.

## Checked

Unset variables leave the arms as they are. `option_env!` yields `None`, so `EXACT_HTTP_WORKERS` stays 6 and `EXACT_HTTP_IN_FLIGHT` stays 32 (`executor_core.rs:17`, `executor_async.rs:40`). The log's first check reads the environment once, caches "no file", and does not open or write anything. The session returns before it builds the strings (`darwin_http.mm:230`).

The const parser rejects an empty string, `0`, a non-digit, and a value past `usize` with `assert!` / `checked_mul` / `checked_add`. Those fail the build in every profile, because the constants are const-evaluated; release's `panic = "abort"` does not bring the old wrapping behavior back. The runtime test covers `None`, `7`, `32`, `""`, `0`, `7a`, and `18446744073709551623`.

JSON goes through `serde_json::json!`, so quotes, backslashes, and control characters in a URL stay inside one JSON string. One `Mutex<File>`, opened once, wraps `write_all`, so concurrent callbacks from the session queue and the lane queues do not interleave lines. Callback file I/O while the log is on is acceptable for this build: it is off unless the variable is set, the exchange lock is released before the write, and the write is one short line. On LTE that is noise next to the request, and it does not take the state lock.

Nil metrics, a nil `last`, and nil addresses take the null path. A task the exchange has already dropped is still the delegate argument for the duration of the call, and Rust copies the C strings before it returns. URLs are written only when `IBEX2_HTTP_METRICS` names a file. The session arm's `queued_ms` is null because that exchange has no resume timestamp; the lane records resume-to-fetch.

READY
