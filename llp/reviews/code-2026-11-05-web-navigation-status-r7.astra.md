# Code review: navigation HTTP status, round 7, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec resume` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `4be9afb621` (parent `536dddbae1`; source base `21c5dfbdf6`).
- **Method:** one follow-up brief (sha256 `3eeb39165fea4252fbb7271ebc0e34baa28b983a64def85389caa6e6b8a8441d`), the round-6 reviewer resumed, four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

**No remaining findings. All earlier blocking settlements are accepted.**

I reviewed `4be9afb621` against `21c5dfbdf6` and the delta from `91157b40f9`, completing correctness, simplification, writing and verification passes.

The I/O lifetime settlement closes round 6’s gap. [ts-fetch.js](/tmp/exact-web-status-review-OEBQRl/checkout-r7/host/web-js/ts-fetch.js:36) counts operations independently of answers and tickets; the [generated renderer](/tmp/exact-web-status-review-OEBQRl/checkout-r7/host/web-js/build.mjs:570) combines that count with unfinished answers for waiting, settlement and recovery.

My read-only probes against the actual runtime modules confirmed:

- Ordinary and held fetches survive synchronous source returns and resolved answer promises: 404 after release, or 503/unsettled at the deadline.
- Discarded and unowned work remains accounted for; abandoned timers retire; status-free pages retain their early response.
- Overlapping operations, rejection and synchronous throw balance the count. Ticketless held completion succeeds.
- Checkpoint revalidation restores store dependencies and reaches 404 without interaction.

In-memory negative controls reproduced premature 200 without I/O accounting and the exception without the ticket guard. Loading the original base’s held module also reproduced that exception, confirming it is an **existing defect repaired here**. Removing checkpoint revalidation prevented recovery.

The earlier compiler, navigation ownership, ID-shadowing, constant-region, store/watch, cache-header and activation settlements remain accepted. I found no required simplification or writing changes, and no public Contract syntax change.

Deferring the answered-open-stream settlement and public `serve` Retry-After forwarding defects remains sound: both are separately recorded pre-existing issues, not claimed fixes.

Verification limits: this checkout has no dependencies or build artifacts. I did not rerun compiled browser suites, root/compiler gates or bundle measurements. Author logs report 78 runtime tests passing and document/build results of 50 passes plus three Firefox startup failures. Root tests still hit the Apple worker failure; the earlier native held-write timeout and two caps violations remain limits. My filtered repository test and diff checks passed. No files were changed.