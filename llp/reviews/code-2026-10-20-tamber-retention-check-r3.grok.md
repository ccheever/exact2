# Code review: LLP 1121 stage 12 B1 follow-up, the background whole build, round 3 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `5dcbc4b358`.
- **Method:** one brief (sha256 `ae2544292f7a8b11da7c2a1682d652269029f74927aad2d5a2ea58f296a0cb13`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (1 NIT).
- **Disposition:** nits taken where cheap (the commit message).

## NIT

- `contract/cli/src/tamber_check.rs:270-272`. A result equal to `sent` clears `sent` while that same job can still be queued (A running, B sent, A sent again). The running result clears the slot; the queued copy is still built, and the next answer can send one more. The fingerprint is the same, so the verdict does not move.

- `contract/cli/src/tamber_serve.rs:485-490`. A scrub frame whose on-disk retention key has left the scrub's key calls `failed` and does not `poll`. A `Failed` result already in the channel leaves those frames `live` until some other request polls. A gesture does not write the file, so the matching arm goes through `known` and does poll, and the next propose records the failure on the retention key. The result is not dropped.

- `contract/cli/src/tamber_check.rs:282-284`. `done` keeps 64 completions. Dropping a remembered failure while the verdict stays `Failed` builds those files once more. The answer stays `rebuild`. The same holds for `made` past 64 keys (`tamber_check.rs:237-238`): non-failed verdicts are dropped, and the key being inserted is put back. The synthetic cache is one slot, filled on the miss path that calls `made`, so a cache hit is not a key whose verdict was just discarded.

- `contract/cli/src/tamber_check.rs:113-160` and `:174-191`. Entries that are not walk inputs are length and mtime. A content change that keeps both, or a file that changes and reverts between the two fingerprints, is judged on the endpoint bytes. Ordinary saves update mtime. Sources, designs, `app.json`, consulted manifests, and icons are content digests.

- `docs/tamber-protocol.md:683-685`. The unknown closure now lasts until a whole build of the files as they are now passes, which is the fingerprint gate. It does not say the breaking write itself stays `live` until that build is polled. The bench figures are only in the commit message; this review did not remeasure them. They are reply times, and they sit where a retention answer plus a background whole build would put them.

## Round 1 and round 2

Closed, as the commit message says they were taken:

- A failure is stored on the retention key when the build's own before and after fingerprints match (`tamber_check.rs:276-278`). `Lost` changes nothing (`:273-275`) and the next unverified or failed answer sends the files as they are now. Two writes with no propose between still reach `rebuild`.
- The single next-job slot is gone. The worker keeps the newest job waiting (`:220-223`). A, then B, then A builds the newest, and a failure under the key stands.
- `compile_path_all` panicking is `Built(false)` (`:187-188`). A dead worker clears `worker` and `sent` (`:257-265`); the next unverified or failed answer starts another. No thread means `rebuild` (`:343-345`, `:319-321`), not a compile on the request path.
- Scrub frames call `known_under` with the key the scrub opened under (`tamber_scrub.rs:76-80`). Files are built only while the on-disk key is still that key (`tamber_serve.rs:485-488`), so a pass of some other retention cannot clear it. `read` does not set `Passed`; `known` does, and only when the job just taken from the current files has that fingerprint in `done` as true (`tamber_check.rs:311-314`).
- A completed `(key, fingerprint, false)` is not sent again (`:316-317`). The worker pulls the next job itself.
- The log line is written after `results.send` (`:224-228`). `began` is logged after the first fingerprint matches (`:174-177`). The tests wait on `seen` and `until`, not on a build count.
- The fingerprint records directories, link targets, and a linked directory walked once (`:145-154`), plus `Walk::facts` (package.json digests and `node_modules` entries). An empty `strings/` directory and a link-target edit are unit-tested. The round-2 symlink-strings hole is closed for an edit that changes the target's length or mtime.

## What checks out

After a failure has been polled, a later propose under that key is `rebuild` until a passing build of the current fingerprint clears it. A lost result, a pass of any other fingerprint, and a superseded job do not clear. Failure is on the retention key, so a value edit while that key is failed stays `rebuild`. The answer that starts the check is the stopped build's: the second `known` after `made` is discarded (`tamber_serve.rs:457-459`), and an unverified key returns live. That is the ruled window. A same-key value edit after `Passed` (`one = 1` to `one = 0`) stays live and starts no check (`tamber_check.rs:307-308`). Main's synthetic cache-hits the same way; `QUEUE.md` records it.

The same stable fingerprint is built once. `Lost` is asked for again. A tree that stays still produces one `Built` and stops. `compile_path_all` does not write the app tree, so a check does not make its own after-fingerprint miss.

One worker for the server's life. The `JoinHandle` is dropped at spawn (`:340-344`). Answers use `try_recv` and an unbounded `send`. Close drops the job sender and does not join; the 30s delay against a 5s close covers that. A send after the result receiver is dropped returns and the worker exits (`:225-226`).

The worker is a separate thread. Retention stop and `PROGRAM` are thread-local (`retained.rs:55-98`); the worker calls `compile_path_all` directly, so it does not stop at retention. Lowering state in `contract/lower` is thread-local. Kernel intern tables are mutexes. Wide-colour sweep does nothing while no holder is registered (`kernel/src/style/wide/reclaim.rs:241-247`), which is this process. No compile in `contract/` or `kernel/` changes the working directory or the environment. Shared colour capacity is process-wide contention, not a wrong verdict on this path.

`Walk::inputs` (`tamber_walk.rs:113-117`) and `digest_of` (`:52-54`) are the walk's files by content: the root, `app.json`, Contract sources, designs (including package `.tamber` files the graph loads), consulted manifests, and icon SVGs. `Walk::facts` adds the package.json files above sources and each `node_modules` entry with its canonicalize target. Strings, fonts, sounds, `.shells`, and `app.ts` are directory stamps. Fonts and sounds that resolve outside the app are refused by the compiler.

The press-scale assert that the propose right after the move is `live` is that propose's own `known`: `poll` runs before `send`, so that call cannot observe the job it just queued. The propose right after `zero = 2` is `rebuild` because the verdict is still `Failed` and the new fingerprint is not in `done`; the transaction planned the pre-write snapshot, so it did not start that check. The raced-check test writes the file only after `began`, inside a 1500ms delay. The lost-check write is one cache-hit transaction against an 800ms sleep that starts at the second `began`.
