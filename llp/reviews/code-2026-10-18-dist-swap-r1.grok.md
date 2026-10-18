# Code review: builds put in place in one step (the dist swap), round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the dist-swap commit (07982a7d0).
- **Method:** one brief (sha256 `bbad1b5899eb8ffd4e26f5474a4fc9af08e5008cba70cc91bf98934928215a57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (case/NFD aliases, a link stage, `.previous` removed before the stage check, the fallback race assertion, the failure log, staticFile).
- **Disposition:** fixed in round 1's commit (26d0dd87d; see its message).

---

Nothing here is blocking. The macOS exchange path works: `renamex_np` with flag `2` (`RENAME_SWAP`) swaps two real directories and leaves `dist` in place. The Linux declarations match `renameat2` and the raw `syscall` path (x86_64 `316`, arm64 `276`, `AT_FDCWD` `-100`, flag `2`). On x86_64 and arm64 the libc `syscall` wrapper shifts those five arguments into the kernel registers; the unused sixth register is ignored. Node has no `import.meta.require`, the `TypeError` is caught, and the two-rename fallback runs. The lock is released on both the early return and a throw. A missing `dist`, a `dist` that is a file, and `EXDEV` all end with the new tree in place or the old tree restored.

1. **should-fix** — `host/web/serve.mjs:671`. `buildTreeFile` treats `realpath(dist) !== resolve(realpath(dirname(dist)), basename(dist))` as "the tree just moved" and retries until it returns null. On the default macOS volume that comparison is also false for a stable directory when the last component differs in case or Unicode normalization. Reproduced: a directory created as `dist` with `index.html` in it, opened as `Dist`, returns null on all four attempts, so every JS-target request is a 404. The same happens for NFC `café` versus the NFD name APFS stores. Retry only when `lstat(dist)` and `lstat(root)` are different devices or inodes (the directory that was looked up has been renamed away). If they are the same directory, serve it.

2. **should-fix** — `host/web/serve.mjs:607`. `existsSync(stage)` follows a symlink, and `renamex_np` / `renameat2` rename that link rather than the directory it points at. Reproduced: a symlink stage whose target holds `index.html` of `new` leaves `dist` as a symlink (`lstat` is not a directory) and `rmSync(stage)` deletes the previous real tree. `buildTreeFile` still follows the link, but `publishedFile` opens `resolve(dist)` through the filesystem helper, which uses `O_NOFOLLOW` and refuses a linked root. Require `lstat(stage)` to be a real directory before exchanging, and the same for `dist` when it exists.

3. **should-fix** — `host/web/serve.mjs:602`. The new "no build to put in place" check runs after `rmSync(dist.previous)`. If a two-rename was killed after `dist` moved to `dist.previous` and the next `replaceBuild` is called with a missing stage (`dev.mjs` has no startup restore; `build.mjs:111` is only the wasm/bake process), the only remaining tree is deleted and then the call throws, so `dist` stays absent. Stat the stage and require a real directory before deleting `.previous`.

4. **should-fix** — `host/web/replace-build.test.mjs:50`. The two-rename test requires a reader to lose the race (`miss + err > 0`). That is not an invariant: three runs of 80 swaps with `EXACT_DIRECTORY_EXCHANGE=0` scored `err` 2, then 0, then 3 (`miss` was 0 every time). A zero run fails CI. Assert the final tree (`<p>400`) and that a forced mid-swap failure restores `dist`; drop the "someone must have lost the race" assertion, or loop until a miss is observed and only then check that the final file is whole.

5. **nit** — `host/web/serve.mjs:646`. Any failed exchange (`EXDEV`, `ENOENT`, `ENOTDIR`, a one-off `EFAULT`) is logged once as "no one-step directory exchange here", while later calls still try the syscall. Log only for `ENOTSUP` / `ENOSYS` / `EOPNOTSUPP`, and keep trying when the pair is simply on two volumes.

6. **nit** — `host/web/serve.mjs:389`. Only `buildTreeFile` retries when `realpath` follows a directory that has just been renamed to the stage path. `staticFile` (wasm `serveStatic`) can still return that stage path. `staticRead` retries a failed byte read, so the HTTP server usually recovers; a direct `staticFile` caller can hand out a path `rmSync(stage)` is about to delete. Use the same inode check there.
