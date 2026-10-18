# Code review: builds put in place in one step (the dist swap), round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the dist-swap commit and round 1 (26d0dd87d).
- **Method:** one brief (sha256 `ff4cbe435884e772dfd8d0218e31cf45f5ada1c4ea7ac1af7667a4b10d49fd5e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (nested retries, an absent dist losing `.previous`, the exchange probe, bigint inodes, the log).
- **Disposition:** fixed in round 2's commit (99dde2cbf; see its message).

---

Nothing is blocking. The seven round-1 fixes hold on the paths they name: a case or NFD alias of `dist` serves (device and inode, checked here), a transient miss is asked again, a link stage is refused before `.previous` is removed, a lone `.previous` survives a missing stage, Node falls back and still installs the new tree, and macOS `renamex_np` with flag `2` swaps and returns a number `0`.

1. **should-fix** — `host/web/serve.mjs:401` (multiplied at `host/web/serve.mjs:462`). A stable symlink, a path whose real target is outside the build, or a non-file used to return null on the first look. Those three results now `continue`, so one `staticFile` call does four lookups, and `staticRead` treats null as transient and calls it four more times (sixteen realpaths for one symlink 404, including a missing GPU file on the allowlist). The swap race still needs a retry: `realpath` of a regular file can briefly name the directory the old tree just moved to. **Fix:** `return null` when `lstat` says symlink or non-file, or when the resolved path is outside the root. Retry only `ENOENT` and a regular file whose `realpath` spelling moved.

2. **should-fix** — `host/web/serve.mjs:618`. The stage check runs before `.previous` is removed, but when `dist` is already absent `.previous` is still deleted before `rename(stage, dist)`. That is the state a killed two-rename leaves, and the JS build (`host/web/build.mjs:64`) does not put `.previous` back first (the wasm build does, at `host/web/build.mjs:111`). A second kill in that gap, or a failed rename (stage for a wasm build lives under `target/web-dist-stages`, so `EXACT_WEB_DIST` on another volume is `EXDEV`), throws with nothing to restore. The exit hook then deletes `stage` (`host/web/build.mjs:65` and `:137`). The last good tree and the new one are both gone. **Fix:** if `dist` is absent, rename `stage` onto `dist` first and delete `.previous` only after that rename succeeds.

3. **should-fix** — `host/web/serve.mjs:634`, `host/web/replace-build.test.mjs:40`. `directoryExchange()` is true when the libc symbol loaded, including when this volume returns `ENOTSUP` or `EXDEV`. The exchange test is not skipped there. With the default 20 ms gap it can also pass on the two-rename fallback without an exchange ever succeeding. **Fix:** probe by swapping two temporary directories (or read the volume capability) and skip when that probe fails. In the stress test, assert the writer exchanged (no `.previous` left behind, or a counter from `replaceBuild`).

4. **nit** — `host/web/serve.mjs:408`. `sameFile` compares `stat`’s `dev` and `ino` as numbers. Past `2^53`, distinct directory inodes round to the same number, so the check can accept the old tree it exists to reject. `host/web/dev.mjs:627` already stats with `{ bigint: true }`. **Fix:** compare `BigInt` device and inode.

5. **nit** — `host/web/serve.mjs:665`. The new text is right for “no syscall here” and for `EXDEV`. It still says the build is put in place by two renames before that fallback runs, and it uses the same sentence for `EPERM`, `EACCES`, and `ENOTSUP`. **Fix:** log the errno, and log the two-rename sentence only after that path returns.
