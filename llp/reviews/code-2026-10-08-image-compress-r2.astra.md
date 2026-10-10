**One blocker and two should-fix queue issues remain.**

1. **Blocker — an awaited ordinary write can still overwrite a newer write after timing out.** [prelude.js:1427](/private/tmp/imgrv/wt-c2/js/src/prelude.js:1427)

   `__exact_storage_failed` still calls `landed()` unconditionally, unlike the guarded background handler. An awaited `atomicWriteFile("x", old)` stalled in `sync_all` therefore releases the queue after 30 seconds. A queued write stores `new`; the original rename subsequently restores `old`.

   I reproduced `new → old` using the actual prelude and a deferred provider. This behavior predates the branch, but remains contrary to A1.5/A1.9’s claimed safety condition and the requested answer-path check.

   **Fix:** advance only the specific compression whose commit permission was revoked. Preserve an ordinary operation’s head and completion owner until it actually finishes, even if its answer fails. Add a deterministic stalled-write test covering the answer path.

2. **Should-fix — a discarded waiter can consume the abandonment notification needed by the let-go waiter.** [fs_image.rs:60](/private/tmp/imgrv/wt-c2/vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:60), [storage.rs:126](/private/tmp/imgrv/wt-c2/js/src/storage.rs:126)

   Start an awaited compression and dispatch its waiter, then supersede the answer while decoding remains blocked. `finish_let_go` starts another waiter. The original waiter reaches its deadline first, revokes the gate, and returns `IMAGE_ABANDONED`; its outcome is discarded. The later waiter now gets `Nothing`, returns the generic timeout, and never triggers `failAbandonedImage`. The compression and subsequent storage remain stuck.

   Native continuation work continues after being forgotten; its result is discarded by [executor_core.rs:511](/private/tmp/imgrv/wt-c2/host/apple/src/executor_core.rs:511). The new supersession test misses this interleaving because it never dispatches the first answer’s waiter before replacing it.

   **Fix:** associate abandonment with the operation and acknowledge it when its actual owner settles it. Transfer or retire stale waiters when ownership changes. Test supersession with the first waiter already running and a barrier-controlled codec.

3. **Should-fix — failing one let-go compression discards ownership of its other queued operations.** [prelude.js:1355](/private/tmp/imgrv/wt-c2/js/src/prelude.js:1355)

   Suppose an answer issues compression and another storage operation before awaiting both. After supersession and compression timeout, `failAbandonedImage` starts the second operation. The following loop nevertheless marks every let-go call lost and removes it, despite its remaining storage.

   The actual-prelude probe leaves `queued: 1`, `inFlight: 1`, `head: false`, and no let-go work reported. The native executor has no owner delivering the second operation’s completion, so subsequent saves remain queued.

   **Fix:** settle only the abandoned operation; retain or transfer ownership of remaining operations until they finish. Extend the test to multiple operations from the same forgotten answer, and multiple forgotten answers.

The round-one background overwrite regression is fixed: an ordinary background timeout now preserves its queue. The web deadline check and non-Apple test declarations are also fixed. The web size check matches the amended guarantee—before Blob construction and decoding, while IndexedDB still reads the entire record. The let-go fix remains incomplete for findings 2 and 3.

I found no additional material CF ownership or FFI defect in `apple.rs`; owned references, dictionary retention, and the autorelease pool are balanced. Search behavior matches the local Bluesky loop in 48 cases, with the documented deviations. Host dispatch, worker clock capture, Rust requests, refusal wiring, and Linux’s missing-codec path otherwise match A1.

Five Bun image-helper tests, caps, boot, and diff whitespace checks passed. Native/Hermes, simulator, and real-browser integration tests were not run. No files were changed.

**Yes. Finding 1 blocks landing.**