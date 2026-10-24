# Code review: filesystem reader retry, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `59bd54153a`.
- **Method:** one brief (sha256 `353613aa9f78fd81787305889e924117bfd04ab7309ea438566c6ab4bfe18893`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `scripts/install-page.test.mjs:288` — The close-generation guard is untested. `get()` and `closeFilesystemReader()` run in the same turn, while the helper is still up, so the pending read is rejected by `session.close()` (`scripts/filesystem.mjs:361`) with `filesystem reader closed`. That error has no `EXACT_FS_READER_EXITED` code, and the catch at `scripts/filesystem.mjs:315` returns it on the code check alone. The round-1 window is the other order: `fail()` (`scripts/filesystem.mjs:334`) has already rejected the read with `READER_EXITED` and cleared `reader`, and `closeFilesystemReader` (`scripts/filesystem.mjs:303`) runs before the catch microtask. That happens when one read's stdout reply and a sibling's child `close` land in the same turn (stdio ends before process `close`, both before microtasks) and the answered read's continuation calls `closeFilesystemReader`. `closes++` runs before `reader?.close()`, and `closes !== at` refuses the re-ask, so that closed read does not spawn another helper. Deleting `closes` still leaves this test green. The `!existsSync('spawns')` arm at line 292 also treats a helper that has not written `spawns` yet as success.

The retry itself is one re-ask per `filesystemRead`: the second `send` has no further catch, and a second exit rejects with `EXACT_FS_READER_EXITED`. `fail` clears `reader` when it still points at that session before the rejection microtask, so the re-ask's `reader ??=` opens a new session. `fail` ignores a second call, and a reply shifts its item off `pending` before resolving it, so a close and a reply settle that promise once. Spawn `error` stays a raw `fail` (no `READER_EXITED`); on this Bun it is emitted before `close`. Unexpected replies and `session.close()` are also untagged. Only `op === 'get'` is sent, and `--serve-reads` only reads. Liveness is the existing per-session transition: `fail` unrefs the dead child, and the fresh helper is referenced only while its retry is pending. `bun test scripts/install-page.test.mjs -t 'resident reader'` passed (1 test). QUEUE's other two halves are already landed: `executable()` runs the digest-named capture (`ef56fda74d`), and `a7656a62f3` references the idle reader only on a liveness transition, so a finished smoke is not held open by the helper.
