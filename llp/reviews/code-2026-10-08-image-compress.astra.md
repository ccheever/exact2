**One blocker remains, plus four should-fix issues.**

1. **Blocker — background timeouts can now let ordinary writes overwrite newer data.** [background.rs:86](/private/tmp/imgrv/wt-c1/js/src/background.rs:86), [prelude.js:1401](/private/tmp/imgrv/wt-c1/js/src/prelude.js:1401).

   The new failure handler rejects and advances **every** background storage operation, but only `compressImage` has a CommitGate. Suppose an unawaited `atomicWriteFile("x", old)` stalls in `sync_all` beyond 30 seconds, with `writeFile("x", new)` queued behind it. The timeout now starts the second write; the first job can subsequently rename over it.

   I reproduced this ordering with the actual prelude and a deferred storage provider: the reported failure remained, while the stored value changed from `new` back to `old`.

   **Fix:** advance only after an operation-specific terminal decision guarantees that the failed operation cannot mutate storage. Keep waiting for nongated writes, or extend commit coordination to them. Add a deterministic stalled-write regression test.

2. **Should-fix — a superseded compression that times out still strands the queue.** [storage.rs:121](/private/tmp/imgrv/wt-c1/js/src/storage.rs:121), [turns.rs:151](/private/tmp/imgrv/wt-c1/js/src/turns.rs:151), [prelude.js:1352](/private/tmp/imgrv/wt-c1/js/src/prelude.js:1352).

   The new abandonment mechanism also runs through `finish_let_go`. Its failure handler marks forgotten calls lost and removes them, but never settles the queue head. Supersede an awaited compression, let its wait expire, then issue another write: the compression cannot commit, but the next write remains queued without an owner delivering the abandoned completion.

   The in-memory reproduction returned `queued:1`, `inFlight:1`, `head:false`; only compression had reached the provider.

   **Fix:** route this timeout through the same head-settlement mechanism, drain rejection reactions, and update progress/accounting. Test supersession during a delayed compression.

3. **Should-fix — the web’s 64 MiB source limit is checked after an unbounded read.** [storage-fs.js:379](/private/tmp/imgrv/wt-c1/host/web/storage-fs.js:379), [storage-fs.js:215](/private/tmp/imgrv/wt-c1/host/web/storage-fs.js:215).

   Picked entries contain Blobs, but ordinary file writes store ArrayBuffers. `store.blob()` retrieves the complete IndexedDB record and constructs a Blob before `image.compress()` checks its size. Compressing an existing 256 MiB byte-backed file therefore materializes its entire payload before returning `too-large`, contradicting A1.5’s pre-read bound.

   **Fix:** use Blob-backed contents or separately readable size metadata checked transactionally before retrieving the payload. Test oversized byte-backed entries as well as picked Files.

4. **Should-fix — the web starts decoding after its deadline has expired.** [storage-image.js:166](/private/tmp/imgrv/wt-c1/host/web/storage-image.js:166).

   There is no deadline check between awaiting the header and calling `createImageBitmap`. If that await takes over 20 seconds—or the worker resumes after that interval—the expensive decode still starts, with timeout detected afterward. An in-memory clock probe confirmed one decoder invocation after expiry.

   **Fix:** check the deadline immediately before decoding. Add a delayed-header test asserting `timeout` and zero decoder calls, as A1.5 requires.

5. **Should-fix — the new integration-test module fails non-Apple warnings-as-errors checks.** [compress.rs:5](/private/tmp/imgrv/wt-c1/js/tests/it/compress.rs:5), [compress.rs:114](/private/tmp/imgrv/wt-c1/js/tests/it/compress.rs:114).

   `Answer`, `Dispatch`, `Work`, and `noise_bmp` are used only by Apple-gated tests, but their declarations compile on Linux, Windows and Android when Hermes is enabled. They produce unused-import/dead-code diagnostics under the prescribed Clippy `-D warnings` run.

   **Fix:** apply the Apple condition to those imports and the helper too.

The four requested round-3 changes are present: gates survive until `take_task`; the waiter rechecks after abandonment; unload/shutdown abandon unwritten compression; and background compression rejection advances the queue. The ANMF offsets are corrected and covered by the passing header test, consistent with [libwebp’s field order](https://github.com/webmproject/libwebp/blob/main/src/demux/demux.c#L304). Findings 1 and 2 concern the surrounding timeout paths.

I found no additional CF ownership or FFI safety defect in `apple.rs`: owned references and the autorelease pool are balanced. The search matches the local Bluesky implementation for all six recorded sequences, with the documented one-pixel and exact-byte deviations. Host dispatch, grant checks, opcode transport and Linux refusal wiring otherwise match A1.

Four Bun tests, the cap check, boot check and diff whitespace check passed. Native builds, simulator runs and real-browser codec tests were not executed in this read-only environment. No files were changed.

**Yes. Finding 1 blocks landing.**