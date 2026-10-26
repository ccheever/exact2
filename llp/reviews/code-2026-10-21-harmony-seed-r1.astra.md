# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `559bd58e87`.
- **Method:** one brief (sha256 `4bcb9e7ffb3b676dfb0d2637948306ba98f42a777925535433090a30559a8b9b`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**Blocking issues remain in `559bd58e87`.**

1. **P1 — Visible queries do not prove an empty store.** [apps/harmony/seed.mjs:230](apps/harmony/seed.mjs:230)  
   A server containing only custom emoji returns empty `people` and inbox results, so seed erases it without `--force`. Creating a channel before any profiles exist also leaves a room with no memberships, invisible to every inbox. `/dev/status.personas` lists declared development personas, not every stored principal. **Fix:** use an authoritative occupancy check covering everything reset destroys, independent of query visibility. Add memory-server cases containing only emoji, an unjoined room, and data inaccessible to the declared personas.

2. **P1 — Flags before an explicit origin silently target the live default.** [apps/harmony/seed.mjs:240](apps/harmony/seed.mjs:240)  
   `bun exact.mjs seed --force http://127.0.0.1:5500` becomes `--post --force URL`. `arg('--post')` returns null, the supplied URL is ignored, and `force` authorizes erasing **4400**. Bare `seed --force` is parsed as intended; the problem is silently discarding an explicit target. **Fix:** parse flags and the origin together, or reject unsupported ordering and unconsumed arguments. Validate the origin before any side effect and test both argument orders against isolated servers.

3. **P1 — Unavailable query results count as empty.** [apps/harmony/seed.mjs:226](apps/harmony/seed.mjs:226)  
   The pinned Snapback source explicitly returns HTTP 200 with `{loading:true, complete:false, data:null}` for unavailable queries. `body.data ?? []` converts that uncertainty into emptiness; if the remaining probes also produce no rows, reset proceeds. Missing or malformed `data` is similarly unchecked, and `/dev/status` lacks an HTTP-status check. **Fix:** reject unsuccessful status responses and require validated, complete array results before concluding emptiness. Test loading, incomplete and malformed responses and assert that reset is never requested.

4. **P1 — The check and destructive reset are not atomic.** [apps/harmony/seed.mjs:243](apps/harmony/seed.mjs:243)  
   Another client can write after its relevant query was checked—or after all checks—and line 248 deletes that write without `--force`. Concurrent seed processes have the same problem. **Fix:** enforce “seed only if empty” inside the server’s reset transaction. Bind the request to the inspected server/store as well; Snapback already supports `x-snapback-dev-instance` for listener replacement, though that alone cannot prevent concurrent-write loss. Add a deterministic test inserting a write between inspection and reset.

5. **P2 — Timeout cleanup is not guaranteed.** [scripts/harmony-seed.test.mjs:24](scripts/harmony-seed.test.mjs:24)  
   Readiness waits only for a stdout line, without rejecting on child exit/error or a bounded startup deadline. Bun test timeouts can skip this body’s `finally`, as the existing `test-children.mjs` documents. Supervisor tracking only helps when the parent process exits. The synchronous child calls also lack timeouts. **Fix:** use the existing owned-child helper with an `afterEach` cleanup backstop, bounded readiness and subprocess waits, and temporary-directory cleanup independent of normal completion.

6. **P2 — The two override assertions can mask each other’s failure.** [scripts/harmony-seed.test.mjs:45](scripts/harmony-seed.test.mjs:45)  
   `--force` and `--reset` run consecutively, with restored data checked only afterward. Either flag could become a successful no-op while the other performs the reset, and this test would pass. **Fix:** write a distinct sentinel before each invocation and verify its removal immediately afterward.

Auth refusals, explicit `denied` responses, non-successful query HTTP responses and thrown network errors currently stop before reset. The async glue registration is correct; I found no additional blocker in the QUEUE entry.

Static review only: no files changed and no tests, servers, installs, seeds or network commands run.
