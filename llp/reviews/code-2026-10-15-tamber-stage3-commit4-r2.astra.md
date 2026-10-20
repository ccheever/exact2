# Code review: LLP 1121 stage 3, commit 4, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `08203a2830d7df045821f2a4bd0d0a0c223302161cf418e659e50b0d7b6a4a1e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: six medium, one low; Grok: two medium, four low.
- **Disposition:** fixed in round 2; commit 5's own events (held proposals) are commit 5's

---

No high findings. Six medium findings and one low finding remain.

1. **Medium — a hand edit to the written file still loses attribution.** [scripts/tamber-server.mjs:248](/tmp/rv-s3c4/scripts/tamber-server.mjs:248)  
   If the announced digest is A, a hand edit produces B, and a client reads B and writes C before the watcher runs, subscribers receive only C attributed to that client. `written` separates changes to *other* files; it cannot expose B. I reproduced this with the handler. Return the transaction’s before-digests and announce any unobserved changes before publishing the client write. Commit 5 needs this distinction for its foreign-change guard.

2. **Medium — changing the file set can crash a read.** [contract/cli/src/tamber_serve.rs:568](/tmp/rv-s3c4/contract/cli/src/tamber_serve.rs:568)  
   `read_file()` enumerates files, then `design()` enumerates them again through `snapshot()`. If a secondary file disappears between those scans, validation succeeds but the subsequent `.expect("a design file")` panics, taking the server down. Select and validate the requested file against one snapshot, returning an error when it is absent.

3. **Medium — an initial `hello` error permanently disables watching.** [scripts/tamber-server.mjs:145](/tmp/rv-s3c4/scripts/tamber-server.mjs:145)  
   An unreadable/non-UTF-8 design, or an initial request timeout, returns before any watcher is installed while allowing `ready` to resolve. After the file is repaired, ordinary requests can succeed, but nothing triggers another refresh. An in-memory probe confirmed a successful later `hello` with zero watchers. Install watches independently of the first successful snapshot and retry initialization, or fail startup explicitly.

4. **Medium — nested designs leave new root files unwatched.** [scripts/tamber-server.mjs:149](/tmp/rv-s3c4/scripts/tamber-server.mjs:149)  
   With `"tamber": "design/palette.txt"` and no root `.tamber` files, only `design/` is watched. Rust discovers additional designs in the app root, so creating `brand.tamber` there produces no event until some watched file changes. Always watch `appDir` as well as the directories containing existing design files.

5. **Medium — disconnects during startup still leave requests unsettled.** [scripts/tamber-server.mjs:200](/tmp/rv-s3c4/scripts/tamber-server.mjs:200)  
   The handler awaits `ready` before installing body/close listeners. A request that closes during that wait has already emitted its terminal events when `body()` starts, so its promise never resolves. The subscribe branch can likewise register an already-closed response. I reproduced the unsettled request. Observe cancellation before awaiting readiness and check closed state before reading or registering a subscriber.

6. **Medium — `close()` can report success while the child remains alive.** [scripts/tamber-server.mjs:267](/tmp/rv-s3c4/scripts/tamber-server.mjs:267)  
   The five-second timeout resolves successfully without confirming termination or escalating the signal. Both dev loops then exit even if the design server still holds its lock. A controlled-child probe confirmed this behavior. Escalate on timeout and await termination; if cleanup cannot be confirmed, report failure instead of fulfilling the lock-release promise.

7. **Low — the attribution regression test still depends on timing.** [scripts/tamber-server.test.mjs:165](/tmp/rv-s3c4/scripts/tamber-server.test.mjs:165)  
   “Too soon for the watcher” assumes the HTTP write starts within the debounce window. If the watcher wins, this test passes without the `written` fix. Use controlled child replies or a watcher barrier to force the intended ordering.

`exits.push(tamber?.close())` is correct, including the `undefined` case. I found no ordinary FIFO refresh/write ordering problem. In-memory checks passed for 256 simultaneous pollers, retention-boundary detection, and cancellation after polling starts. The detached POSIX group target is appropriate; pinned Bun rejects the fake child’s missing PID. Rust explicitly refuses Windows, and an inverse targeting a vanished secondary file correctly conflicts.

Caps passed all eight categories, and `git diff --check` passed. No files were changed; I did not run dev loops or file-writing tests.
