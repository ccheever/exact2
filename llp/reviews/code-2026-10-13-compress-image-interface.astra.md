# Code review: storage.fs.compressImage's app interface again, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `7c76934f7e469f420519bca195e5dd2953f01943a6eec04a8f4d22b835ee1238`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 2's code; the selected-document codes (outside this API) in QUEUE.

---

HEAD correctly restores the app signature and result type. No other storage declarations changed. The broader audit found these remaining issues; the runtime regressions below originate in `f31c6e2ba`.

- **MAJOR — Oversized images lose `too-large`.** [vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:498](/Users/admin/x2p2/apps/exact2-comp/vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:498): `read_bounded` rejects a source over 64 MiB with `filesystem: storage result exceeds its byte limit`. The prelude maps that to `"failed"`, so documented `e.code === "too-large"` handling no longer runs. Preserve the image-specific code and test oversized input through the job/prelude path.

- **MAJOR — Some deadline failures lose `timeout`.** [vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:217](/Users/admin/x2p2/apps/exact2-comp/vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:217): when a successful codec trial finishes after the deadline, publication fails with `compressImage: deadline expired before publication`, without the `timeout:` component required by `IMAGE_CODE`. Deadline expiry during source reading likewise produces a generic failure. Both reach apps as `"failed"`. The codec’s correctly coded checks and the waiter’s timeout message do not cover these paths.

- **MINOR — Selected-document operations lose filesystem codes.** [vendor/ibex/crates/ibex2/src/stdlib/documents.rs:219](/Users/admin/x2p2/apps/exact2-comp/vendor/ibex/crates/ibex2/src/stdlib/documents.rs:219): `readdir` and `mkdir` on a selected file now return `PermissionDenied`, ultimately `"failed"` in the prelude. Previously they yielded `ENOTDIR` and `EEXIST`, respectively; the web still does. Preserve those operation-specific failures to maintain the documented cross-host codes.

- **MINOR — The restored limits comment omits the scratch-memory limit.** [vendor/ibex/crates/ibex2/src/bindings/storage.d.ts:32](/Users/admin/x2p2/apps/exact2-comp/vendor/ibex/crates/ibex2/src/bindings/storage.d.ts:32): `too-large` also covers the new 512 MiB codec reservation. A 7000×7000 image with `maxDimension: 7000, maxBytes: 2_000_000` exceeds that estimate while remaining below both stated source caps. Include this refusal. The adaptation description is accurate; 20 s is a cooperative worker budget, so an executing codec call can finish later.

- **MINOR — The default pin leaves semantic gaps.** [contract/cli/tests/it/typescript.rs:141](/Users/admin/x2p2/apps/exact2-comp/contract/cli/tests/it/typescript.rs:141): deleting `CompressedImage.size` or adding the positional overload still passes the pin. Both fail the semantic fixture. The storage fixture took approximately **0.11–0.16 s** through the pinned TypeScript 7 API, making a default semantic check worth using instead of substring assertions.

Read-only validation: the current pin and 16 native codec tests passed using existing binaries. The storage fixture passed against generated HEAD declarations and rejected the parent declaration. Executing the prelude classifier confirmed the error-code mismatches; filesystem integration tests were not rerun.

**NOT READY**
