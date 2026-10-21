# Code review: compressImage round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `dd43c3abb3743b99c21fb485b2f9ae291895be8e98d28ddb26526e79fcd832f0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

No findings.

The two commits restore the app contract and the codes that contract names. `f31c6e2ba` changed only `compressImage` in `storage.d.ts`; the other storage methods are unchanged. The declaration again matches the prelude, the docs, the web hosts, and the Rust operation: options in, `{path, type, size, width, height}` out.

The doc comment matches the job. Limits are 1–8192 and 1–67108864, the same bounds the prelude and `exact_data::image` enforce. `too-large` covers a source over 64 MiB, a header over 64 Mi pixels, and the codec's scratch reservation (`data/src/image/apple.rs`). `timeout` is `TRIAL_BUDGET`, 20 seconds. The comment stops at that app contract.

The runtime mapping holds. A bounded read that hits `RESULT_LIMIT` becomes `compressImage: too-large:`, which `IMAGE_CODE` accepts; the Apple test in `data/host/src/tests.rs` asserts that prefix. A trial deadline during the source read, in `WorkControl::check`, or before publication becomes `compressImage: timeout:`. Publication stores that outcome and `settle` returns it, so the caller's `abandoned before publication` does not replace it. The selected-file `readdir`/`mkdir` change stays in `QUEUE.md`, outside this API.

The default-run pin is the right gate. `compress_image_is_declared_as_the_docs_give_it` is an ordinary test in `contract`, a default member, so the six checks run it. It requires the documented signature, refuses the positional form, and checks the call shape in `docs/reference.md`. The `tsc` fixture needs Bun and the pinned compiler, so it stays `#[ignore]` on the async lane.

READY
