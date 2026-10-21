**Two should-fix findings remain.**

1. **Should-fix — a discarded waiter can abandon a later compression.** [storage.rs:126](/private/tmp/imgrv/wt-c3/js/src/storage.rs:126), [task.rs:622](/private/tmp/imgrv/wt-c3/vendor/ibex/crates/ibex2/src/task.rs:622).

   The waiter retains the session, not the operation it originally awaited. On expiry, `abandon_image_work()` revokes every currently registered compression.

   Concrete interleaving: supersede an answer whose ordinary write completes at 29 seconds. The let-go delivery consumes that completion and starts a queued compression before the original waiter wakes. That waiter can miss the consumed completion, continue watching the busy session, and revoke the new compression at its original 30-second deadline—after the compression has run for only one second. Discarding the old waiter’s outcome does not undo that revocation.

   **Fix:** bind abandonment to the operation/gate captured by that waiter; retain abandonment of all operations for teardown. Add a barrier-controlled regression where the first operation completes and its successor starts before the discarded waiter resumes. The new test instead expires the first waiter before supersession, missing this case. This finding is a code interleaving, not a reproduced native stress-test result.

2. **Should-fix — the last commit reintroduces the non-Apple warnings-as-errors failure.** [compress.rs:9](/private/tmp/imgrv/wt-c3/js/tests/it/compress.rs:9).

   `SERIAL` is unconditional, but every use is inside an Apple-only test. With Hermes enabled on Linux, Windows or Android, it produces `static SERIAL is never used`, failing Clippy’s prescribed `-D warnings`. I reproduced that diagnostic with the declaration after excluding its Apple-only uses.

   **Fix:** apply `#[cfg(target_vendor = "apple")]` to `SERIAL`, alongside the other timing-test declarations.

The other round-one and round-two fixes hold in the inspected paths. Actual-prelude probes confirmed that ordinary background and let-go writes retain their queue head, abandoned compressions reject with `timeout`, and a let-go call retains ownership of its subsequent write. A1’s revised scoping of the pre-existing answer-path hazard is honest; I am not reporting that behavior again. The web size/deadline checks and negative BMP-width fix are present.

I found no additional material CF ownership or FFI safety defect: owned references, dictionary retention and the autorelease pool are balanced. The search matches A1 and the recorded Bluesky trials, including the declared deviations. Host routing, worker clock capture, Rust requests, refusals and Linux’s missing-codec path otherwise align with the amendment.

Five Bun tests, caps, boot and diff-whitespace checks passed. Native/Hermes, simulator and real-browser integration suites were not run. No files were changed.

**Yes: the non-Apple warnings-as-errors failure blocks landing. Fix the stale-waiter race alongside it.**