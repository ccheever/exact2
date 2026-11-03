# An independent read can wait forever before its timeout starts

**Status:** Closed
**Systems:** Native HTTP scheduler shared by Apple, Linux and the render host (`host/apple/src/executor_core.rs`, included from `host/linux/src/executor.rs` and `host/render/src/executor.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** An independent `fetch` whose ceiling fits the documented range can sit in the lane forever and never reach the network. `exactTimeout` does not end it. The promise stays pending. The same lane's open streams are what fill the budget.

**Cause.** The independent lane holds 64 MiB (`BYTES[1]`). `reservation` charges a collected body twice its ceiling (`bytes + body * 2 + headers`, `executor_core.rs` `reservation`), which the reference already states, so a ceiling near 32 MiB is the largest that can ever start. Admission of a non-stream read charges the request buffers, not that ceiling (`admit`, the `charge` taken from `reservation`'s middle value). The worker starts the job only once the whole ceiling is free (`next_job`); otherwise it waits on the lane condvar. A stream is charged its ceiling for as long as it is open, on this same lane. The deadline is armed inside `execute`, which runs only after `next_job` has taken the job. A request that has not started has no deadline.

**Reproduction.** Hold several independent streams (each socket's ceiling is charged for the life of the socket) so less than twice the new request's ceiling remains in the 64 MiB lane. Issue `fetch(url, { exactIndependentHttp: { maxResponseBytes }, exactTimeout: 1000 })` with a ceiling the lane cannot currently spare. The fetch is still pending after the timeout. Closing a stream lets it start.

**Fix.** If the ceiling can never fit beside the streams this lane is required to keep, refuse at admission. Arm the request deadline when the job is queued, and fail the queued job when it passes. A ceiling the lane can hold once a stream ends may still wait, and that wait counts toward `exactTimeout`.

**Acceptance.** The fetch above fails within its timeout with `Timeout` or a refusal that names the lane budget. A ceiling that fits beside the open streams still runs. A download with `exactBodyTo` stays charged at the 1 MiB it holds, as it is now.

**Resolution.** Independent non-stream HTTP starts its deadline when queued and passes the remaining time to transport. Queued expiration completes without sending, and admission refuses a ceiling that cannot fit beside open streams.

**Verification after the fix.** The busy-worker queue deadline regression passed in Apple, Linux and render suites. Those suites passed 577, 1713 and 227 tests respectively. Existing file-upload parking semantics are unchanged.
