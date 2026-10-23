# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), narrow re-check 2, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit, Write and Bash denied, a throwaway worktree at `30a3cd9f30`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-read the seed guard at that commit and the snapback contract, then check whether a plain `--post` can still erase a write. No tests, servers, seeds, installs, or network.The guard is in place. Next I’ll check the snapback `/seed` and `/changes` contract against the code paths that still run with no `--force` or `--reset`.**Clean.** An unforced `bun exact.mjs seed` or `seed --post <origin>` does not erase anything someone wrote.

`app.json` runs `seed.mjs` with only `--post`, so the default origin is `http://127.0.0.1:4400` and `--force`/`--reset` stay off. The script reads `/dev/status` (nonempty `storeId` and `instance`, a positive integer `pid`, a persona) and the global head from `GET /changes?since=0` (`seq`, and `store_id` must match). Snapback 0.4.33 treats that `seq` as the committed head. Omitting `wait` is an immediate poll, and a reset is itself a commit, so the head does not return to 0.

If `seq !== 0`, the process prints the refusal and exits before any mutating request. If `seq === 0`, the only write is `POST /seed`: one commit, upsert by id. A refused row writes nothing, and rows that are not in the body are not deleted. A message or profile written after that read uses its own id (`sendMessage` / `setProfile`). A profile that lands first also collides on `principal`, so the seed commit is denied and writes nothing. `/dev/reset` runs only with `--force` or `--reset`, after a second status read with the same instance, store, and pid, and with `x-snapback-dev-instance`.

The tests match that behavior. The real server checks that an empty memory store seeds, and that a later commit is refused with the "nothing was erased" message while Ada's sentinel status remains. The disk server checks the same refusal, then `--force` and `--reset` on either side of the origin. The stand-in checks that a head of 0 hits `/seed` once and never `/dev/reset`, and that a bad status or a replaced instance never resets. Unknown arguments exit 2 before `seed.json` is written.
