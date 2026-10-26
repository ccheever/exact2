# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `4910219e88`.
- **Method:** one brief (sha256 `afe7e9520f918cb26e4b257be1041fd2d978cf5f0b1405765ff598d34042a92c`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two P1 blockers remain.

1. **P1 — Reset can reach a replacement persistent server without force.** [apps/harmony/seed.mjs:263](apps/harmony/seed.mjs:263): both status reads can identify temporary server A, then persistent server B can replace it before the unconditional POST. B receives the reset without `--force`. **Fix:** validate `/dev/status.instance` and send it as `x-snapback-dev-instance` on reset. The pinned Snapback 0.4.33 already rejects mismatches with HTTP 409. Add a deterministic replacement-between-status-and-reset test.

2. **P1 — Populated temporary stores are deliberately erased without consent.** [apps/harmony/seed.mjs:255](apps/harmony/seed.mjs:255): `temporary: true` establishes storage lifetime, not emptiness or ownership. Someone using the README’s in-memory server loses their messages when another agent repeats `seed`. [scripts/harmony-seed.test.mjs:64](scripts/harmony-seed.test.mjs:64) explicitly expects that loss. This violates the lead’s “already holds data” requirement. **Fix:** conservatively require force for resets of existing servers; unforced seeding needs an atomic server-side emptiness check. Update the README and make the populated-memory test require refusal and sentinel preservation.

3. **P2 — Malformed status can pass the identity guard.** [apps/harmony/seed.mjs:242](apps/harmony/seed.mjs:242): two responses containing `{"storeId":"","temporary":true}` pass validation and comparison because both missing PIDs equal `undefined`. The second response’s `temporary` value is also never checked. **Fix:** require nonempty store and instance IDs and a positive integer PID on both responses; reapply the temporary requirement when unforced. Add malformed-response tests asserting zero resets.

The strict boolean comparison itself has no truthiness bypass. Argument ordering/rejections, covered sentinel checks, async registration, and owned-process cleanup look sound by inspection. Test coverage should also assert that invalid arguments create no output file and cover all four flag/order combinations.

No files changed. No tests, servers, installs, seeds, or network commands ran.
