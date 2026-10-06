# Round 1
Found defects in `df7677f15`:

1. **[P1] Wasm TypeScript requests silently lose the deadline.** [js/web/src/lib.rs:332](/Users/admin/projects/exact2-wt-ft/js/web/src/lib.rs:332) reconstructs the request without copying `timeout_ms` from the prelude’s JSON. Consequently, TypeScript `exactTimeout` never reaches the wasm host’s new deadline implementation.

2. **[P1] Web JS body timeouts lack the promised error kind.** [admission.js:43](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:43) returns the browser response at headers. Subsequent `.text()`, `.json()`, or body-reader failures bypass its catch. Reproduced: headers succeed, the body stalls, and reading rejects with `TimeoutError` and no `kind`, instead of `FetchError` kind `"Timeout"`.

3. **[P1] Rustls DNS resolution can outlive the deadline indefinitely.** [rustls_http.rs:207](/Users/admin/projects/exact2-wt-ft/vendor/ibex2/src/transport/rustls_http.rs:207) deliberately invokes synchronous resolution with `NotHappening`. Cancellation is checked only after resolution returns. The deadline thread therefore cannot release a request—or its ordered lane—while DNS is blocked.

4. **[P2] Adding a deadline overrides `Request.signal`.** [admission.js:42](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:42) combines only `init.signal`, ignoring a signal supplied through the input `Request`. Reproduced: an already-aborted `Request` rejects without `exactTimeout`, but succeeds with it.

5. **[P2] Native deadline validation runs after paths that bypass it.** [executor_core.rs:849](/Users/admin/projects/exact2-wt-ft/host/apple/src/executor_core.rs:849) is never reached by streams, handed-off native work, or continuations. Additionally, [data/host/src/lib.rs:121](/Users/admin/projects/exact2-wt-ft/data/host/src/lib.rs:121) replaces storage requests with fresh continuations, dropping their deadline. These invalid combinations execute instead of being refused.

6. **[P2] Timeout classification does not preserve which event won.** [executor_core.rs:879](/Users/admin/projects/exact2-wt-ft/host/apple/src/executor_core.rs:879) can relabel an earlier abort/network failure when the timer fires before classification; successful completion can also slip between setting `passed` and aborting. The web’s [http-body.js:189](/Users/admin/projects/exact2-wt-ft/host/web/http-body.js:189) similarly examines current signal flags instead of the combined signal’s winning reason. Reproduced: deadline wins, caller abort follows before catch, and the result is `Aborted`.

7. **[P2] Deadline-thread creation failure silently removes deadline enforcement.** [executor_core.rs:909](/Users/admin/projects/exact2-wt-ft/host/apple/src/executor_core.rs:909) discards the spawn error. On Apple, the fallback is an idle timeout, so a trickling response can continue beyond the whole-exchange deadline. Other failures are classified `Network`, since `passed` remains false.

8. **[P2] Longer rustls deadlines retain the 10-second connection timeout.** [rustls_http.rs:489](/Users/admin/projects/exact2-wt-ft/vendor/ibex2/src/transport/rustls_http.rs:489) overrides total/receive limits but leaves `timeout_connect(10s)` intact. A 30-second request can therefore fail during connect/TLS at 10 seconds with `Network`, contradicting the stated guarantee that transport limits expire after the deadline.

9. **[P2] The documented Rust support needs qualification.** [logic/abi/src/lib.rs:206](/Users/admin/projects/exact2-wt-ft/logic/abi/src/lib.rs:206) explicitly refuses every timed request crossing the Rust module ABI. That avoids silent loss, but means the LLP’s unqualified `Request::timeout(ms)` claim does not hold for separately loaded Rust modules.

No missing Timeout encoding or TypeScript declaration defect found. Timer threads are detached, but normally exit when `_done` drops; no persistent thread leak found.

Three tests in the existing native test binary passed. Web failures were reproduced with in-memory transport probes. The live transport test was blocked by sandbox restrictions on listening sockets. No files changed.
# Round 2
Five concrete defects remain at HEAD (`4c439fe95`):

- **P2 — Bodyless responses become failures.** [admission.js:56](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:56) constructs a `Response` with an `ArrayBuffer`, even for 204, 205, and 304. Those statuses prohibit a body, including an empty buffer. Adding `exactTimeout` therefore turns valid responses into `FetchError(Network)`.

- **P2 — Response metadata is lost.** [admission.js:56](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:56) also resets `url` to `""`, `redirected` to `false`, and `type` to `"default"`. Deadline-bearing fetches consequently break callers that inspect redirects or resolve relative links against the response URL.

- **P2 — Caller timeouts are still misclassified.** [admission.js:62](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:62) checks the winning reason’s name, not whether Exact’s deadline caused it. An already-aborted caller signal carrying `TimeoutError`, combined with `exactTimeout: 3600000`, immediately reports “the request timed out after 3600000 ms.” Compare the winning reason with the deadline’s actual reason.

- **P2 — Explicitly clearing a Request’s signal is ignored.** [admission.js:44](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:44) treats `init.signal: null` as absent and restores the input Request’s signal. With `exactTimeout`, an aborted Request therefore rejects even when the caller explicitly clears its signal; without the deadline, it succeeds.

- **P2 — Native auth and surface work still bypass validation.** [abi.rs:237](/Users/admin/projects/exact2-wt-ft/host/apple/src/abi.rs:237) routes both before executor admission. An embedded source without the optional storage wrapper can return `Request::auth(...).timeout(10)` or surface work with a timeout, and Apple executes it without refusal. [Linux surface admission](/Users/admin/projects/exact2-wt-ft/host/linux/src/surfaces.rs:465) has the same omission.

Reproduced the four web cases with in-memory probes. Three focused native tests passed using the existing test binary. No files modified.
# Round 3
Two concrete defects in **4b2b79c7a**:

- **[P2] Early timeout refusals bypass the ordered barrier** — [host/apple/src/abi.rs:240](/Users/admin/projects/exact2-wt-ft/host/apple/src/abi.rs:240), also [host/linux/src/presenter.rs:677](/Users/admin/projects/exact2-wt-ft/host/linux/src/presenter.rs:677). These branches record a refusal without setting the executor’s `ordered_barrier`. Given ordered requests A, B with an invalid timeout, and C, C can execute and settle before B: B’s refusal waits for `ordered_idle()`. A stalled C also blocks delivery of B’s refusal. Preserve the executor’s refusal barrier here.

- **[P2] `signal: undefined` incorrectly clears a Request’s signal** — [host/web-js/admission.js:45](/Users/admin/projects/exact2-wt-ft/host/web-js/admission.js:45). With `exactTimeout`, an explicitly undefined signal selects only the deadline signal. Fetch semantics preserve the input Request’s signal for `undefined`; only `null` clears it. Reproduced with an already-aborted Request: omitting `signal` rejects, but `{signal: undefined, exactTimeout: 1000}` proceeds. Check for `undefined` while preserving the explicit-null case.

No files modified. Server-based tests were blocked by the sandbox.
# Disposition

Every finding was fixed, in 4c439fe95, 4b2b79c7a and 613801a01, except two that are written down instead:
- Off Apple, a name lookup is the system resolver's and is not bounded by the deadline; the abort is seen once it returns (LLP 1016 D4's amendment says so).
- A separately loaded Rust module cannot carry a deadline across its seam; such a request is refused, never sent without one (the LLP says so).
Round 3's two findings (an early refusal skipping the executor's ordered fence; `signal: undefined`) and Grok's (the deadline aborting a body that had arrived in time) were fixed after the last round and are not re-reviewed.
