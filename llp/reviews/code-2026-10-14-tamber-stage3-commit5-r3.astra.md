# Code review: LLP 1121 stage 3, commit 5, round 3, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `b6c9b55b441eb54327a6ad4e3f50f6a7242c6c159871757ad4b18268f976049e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: three medium, one low; Grok: two medium, one low.
- **Disposition:** applied without re-review

---

No high-severity findings. Three medium issues and one low issue remain:

- **Medium — [scripts/tamber-commit.mjs:61](/tmp/rv-s3c5/scripts/tamber-commit.mjs:61): Failed status reads become clean baselines.** The result’s `error` and exit status are ignored. Empty or partial stdout makes unreported paths appear clean. If a file was staged but its working tree matches HEAD, the subsequent content comparison passes, allowing commit to overwrite that staging. **Fix:** fail closed when status fails or its output is incomplete.

- **Medium — [scripts/tamber-commit.mjs:68](/tmp/rv-s3c5/scripts/tamber-commit.mjs:68): The new batch reader remains newline-unsafe.** A newline in a filename creates additional `cat-file` requests, shifting responses onto subsequent paths. A probe reproduced an ordinary clean file being permanently classified as dirty. **Fix:** use `cat-file --batch -Z`, NUL-delimited requests, and matching size-aware response parsing.

- **Medium — [scripts/tamber-server.mjs:334](/tmp/rv-s3c5/scripts/tamber-server.mjs:334): Invalid paths throw before protocol validation and can leak holds.** A transaction naming `"bad\u0000.tamber"` causes `spawnSync` to throw an uncaught `TypeError`, rejecting the HTTP handler instead of returning an error response. `prepare()` may already have incremented another pending path’s hold, which then remains unreleased. **Fix:** validate paths before preparing, make preparation exception-safe, and return a structured 400 response.

- **Low — [scripts/tamber-commit.mjs:62](/tmp/rv-s3c5/scripts/tamber-commit.mjs:62): Rename/copy source fields are parsed as status records.** Porcelain `-z` emits a second pathname for these entries. Applying `slice(3)` to it invents another entry—for example, `my-app.tamber` becomes `app.tamber`—which can falsely mark a clean requested path as staged. **Fix:** use `--no-renames` or consume the paired pathname explicitly.

Read-only and in-memory probes covered overlapping holds, failure releases, late successful accepts, server-down replies, and missing/untracked HEAD paths. No files were changed; the integration suite was not run.
