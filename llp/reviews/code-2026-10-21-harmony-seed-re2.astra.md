# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), narrow re-check 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `30a3cd9f30`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Not clean at `30a3cd9f30`: unflagged seeding can still overwrite user edits.

- **P1 — [apps/harmony/seed.mjs:275](apps/harmony/seed.mjs:275): `/seed` remains destructive for matching IDs.** Harmony generates identical IDs each run. If invocation A reads head 0, invocation B seeds, someone edits a seeded message/profile, and A then posts, A overwrites that edit. The [pinned contract](/Users/admin/.bun/install/cache/snapback4@0.4.33@@@1/CLIENT-AND-OPERATIONS.md:3069) explicitly says reseeding updates mutable columns. A replacement listener serving previously seeded data is another route, because this POST omits `x-snapback-dev-instance`. **Fix:** enforce head 0 atomically with the seed transaction, bound to the inspected store/instance, or use an atomic insert-only operation. Send the instance header on `/seed` too; that alone fixes replacement, but not concurrent writes within one instance.

- **P2 — [scripts/harmony-seed.test.mjs:129](scripts/harmony-seed.test.mjs:129): the claimed race test does not test preservation.** Its stand-in always reports head 0 and merely counts `/seed` requests; it neither applies the payload nor introduces a concurrent write. It passes despite the overwrite above. **Fix:** hold A’s POST after its head read, seed a test-owned backend and edit a seeded row, then release A and assert the edit survives or A is refused. Cover listener replacement separately.

Read-only inspection only; no tests, servers, seeds, installs, or network commands run.
