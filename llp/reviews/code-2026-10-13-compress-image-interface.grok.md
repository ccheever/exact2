# Code review: storage.fs.compressImage's app interface again, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `7c76934f7e469f420519bca195e5dd2953f01943a6eec04a8f4d22b835ee1238`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 2's code; the selected-document codes (outside this API) in QUEUE.

---

## Review of `1de597f`

The declaration is the documented app interface again: `compressImage(from, to, options)` resolves to `{path, type, size, width, height}`, and that matches the prelude, both web hosts, and the Rust data host. `f31c6e2ba` changed no other signature in `storage.d.ts`. Option bounds (1..8192, 1..67108864) and the 20 s `TRIAL_BUDGET` match the prelude, `exact_data::image`, and the web codec. The default-run pin catches the signature regression the refresh caused; the `tsc` fixture should stay on the async lane.

The restored refusal sentence does not match the native job.

### MAJOR

`vendor/ibex/crates/ibex2/src/bindings/storage.d.ts:32` — The comment says a source over 64 MiB is `too-large`. On the native path a TypeScript module actually runs, `ImageJob::perform` reads through `read_bounded(..., MAX_SOURCE_BYTES)` (`vendor/ibex/crates/ibex2/src/stdlib/fs_image.rs:498`). A longer file is `std::io::Error::other("storage result exceeds its byte limit")` (`vendor/ibex/crates/ibex2/src/stdlib/fs.rs:78`), wrapped as `filesystem: storage result exceeds its byte limit` (`vendor/ibex/crates/ibex2/src/stdlib/app_fs_unix.rs:24`). `js/src/prelude.js:779` only lifts `too-large` out of `compressImage: too-large:`, so `storageCode` returns `failed`. The codec's own `source.len() > MAX_BYTES` → `compressImage: too-large` (`data/src/image.rs:172`) never runs, because the read already refused the file. The same job serves opcode 121 and a Rust source (`js/src/storage.rs:237`, `data/host/src/native.rs:204`); the Rust host then promotes any message containing that sentence to `Outcome::Failed { kind: Unsupported }` (`data/host/src/native.rs:421`). The web codec still throws `too-large` (`host/web/storage-image.js:169`), and `docs/reference.md:868` still documents that code. A header over 64 Mi pixels is still `too-large` (`data/src/image/apple.rs:274`). The same parenthetical is also short: an image under both stated caps is `too-large` when the estimated scratch exceeds the 512 MiB reservation (`data/src/image/apple.rs:290`).

Map that read-cap refusal back to `compressImage: too-large:`, which is what the web host, the codec, and `docs/reference.md:868` already say. If the generic limit is intentional, change this comment and that docs sentence so a native source over 64 MiB is `failed` with `storage result exceeds its byte limit`, and name the scratch reservation as another `too-large`.

### MINOR

`vendor/ibex/crates/ibex2/src/bindings/storage.d.ts:34` — The new sentences describe the prelude's adaptation to the positional binding. `contract/cli/src/typescript.rs:27` copies this comment into every app's generated `app.contract.d.ts`. The replay rule is already in `vendor/ibex/EXACT-PATCHES.md:66`. End the comment at the app contract: options, record, grants, refusal codes.

### The other checks

No other app-facing storage signature moved in `f31c6e2ba`. The prelude diff there only classifies `document ENOENT` and the other document errnos. `files.compressImage` still takes the options object, checks 1..8192 and 1..67108864, calls the positional binding, and maps `width\theight\tsize` to the record (`js/src/prelude.js:921`). `TRIAL_BUDGET` is 20 s (`fs_image.rs:18`); a deadline inside the codec is `compressImage: timeout:`, which the prelude codes as `timeout`. A missing codec is still `compressImage: unsupported:` before the read (`fs_image.rs:437`).

`compress_image_is_declared_as_the_docs_give_it` fails if the options signature line disappears or if `docs/reference.md` drops the documented call. That is the regression the six checks missed. The `tsc` fixture still needs Bun and the pinned compiler, writes a temp project, and typechecks the whole contract seam, which is why it is `#[ignore]` and `scripts/async.mjs` runs it with `--ignored`. Leaving it there is the right split. The string pin does not notice a positional overload added beside the options form; the ignored `tsc` fixture does, via `@ts-expect-error` on the four-argument call.

NOT READY
