# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit, Write and Bash denied, a throwaway worktree at `559bd58e87`.
- **Method:** one brief (sha256 `4bcb9e7ffb3b676dfb0d2637948306ba98f42a777925535433090a30559a8b9b`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the last commit in exact2 as a read-only pass: Harmony's `seed --post` guard, its test, and the QUEUE entry. No edits, and no test/server/seed/network commands.The review is of `HEAD` in exact2 only. I'll locate that tree and inspect the commit, then read the seed guard, test, and QUEUE entry without running anything.Bash is blocked, so I’ll read git metadata and the seed-related files directly.I’ll trace command-arg passing and the Snapback people/inbox/reset APIs next, then reconstruct HEAD from git metadata.Checking how conform names Harmony’s app directory, and whether `body.data` can be a non-array success envelope.Round 1 on `559bd58e` (`Harmony's seed refuses to empty a server that holds data unless --force (or --reset); QUEUE: Harmony in conform with a backend of its own`). Read-only; no test, seed, server, install, or network.

**Nothing blocks the README path.** `bun exact.mjs seed` becomes `bun ../seed.mjs --post`. On a Harmony server that still has profiles (the live demo on 4400 after a seed), `people()` is public (`schema.q` `profiles[]`), `holdsData` is true, and `/dev/reset` is not sent. Query failures and `/dev/status` parse failures exit before reset.

Agents that add `--force` and then an origin, or that write `--post` again (`exact.mjs seed --post <url>`), can still empty 4400.

---

1. **P1 — `apps/harmony/seed.mjs:214` and `:240`**  
   `arg('--post')` takes only the token immediately after the first `--post`, and drops it if it starts with `--`. `app.json` already injects `--post`, so extra flags and a later origin never bind:
   - `bun exact.mjs seed --force http://127.0.0.1:<memory>` → `--post --force http://127.0.0.1:<memory>` → origin stays `http://127.0.0.1:4400`, `--force` is true, live reset.
   - `bun exact.mjs seed --post http://127.0.0.1:<memory> --force` → `--post --post http://… --force` → same.
   The success line is `${origin}: …`, so this reports 4400 after the wipe.

   **Fix:** Walk argv once. Treat `--force`/`--reset` as booleans; `--json` takes the next non-flag; origin is the first non-flag after any `--post`, or a later non-flag once `--post` has been seen. Keep the default origin only when none was given. Cover `--post --force <memory-origin>` in the test (that shape today would post to 4400).

2. **P2 — `apps/harmony/seed.mjs:219-232`**  
   `holdsData` is only `people` for `personas[0]` plus `inbox` per persona. That misses data the schema still holds:
   - public `emoji` (`customEmoji()`, `schema.q:141` / `:182`)
   - rooms with no remaining membership (`inbox` is `members[viewer]`)
   - lastReads / messages / pins for principals that are not in `/dev/status`’s persona list  
   Missing or non-array `body.data` becomes `[]` (`?? []`), which is treated as empty. `personas: []` leaves `personas[0]` undefined, skips the inbox loop, and if `people` answers empty without throwing, reset proceeds. Harmony profiles cannot be deleted, so a *seeded* 4400 still trips `people`. An empty store that then only got emoji, or a store whose personas list is empty while public reads succeed, does not.

   **Fix:** Throw when `/dev/status` is not ok, when `personas` is missing/empty, and when `data` is not an array. Also ask `customEmoji` (same public read as `people`). Stronger: `GET /changes?since=0` as in `scripts/agent-test.mjs` `backendHead`, and treat a positive `seq` as held (still throw on `!ok` / `denied`). Do not reset on a probe that did not return a list.

3. **P2 — `scripts/harmony-seed.test.mjs:24-50`**  
   The test never uses the exact.mjs argv (`--post` then flags). Every `seed()` call is `--post ${origin} …`, which hides finding 1. It never builds a people-empty, emoji-or-room-only store, so finding 2 is untested. Cleanup is weaker than `scripts/memory-backend-test.mjs` on the same binary: no trim/loopback check on the first stdout line, no race of that line against server exit, stderr discarded, `spawnSync` for `seed.mjs` has no `timeout` (it blocks the JS thread, so the 120s bun timeout cannot fire), `SIGTERM` only, no descendant kill. `SNAPBACK4_SUPERVISOR_PID` only helps after the test process itself dies.

   **Fix:** Copy the memory-backend-test ready/stop sequence (trim, `^http://127.0.0.1:\d+$`, pid + `temporary`, SIGTERM then SIGKILL, URL wait raced with `exit`). Give `spawnSync` a timeout. After the parser fix, add `['seed.mjs', '--json', file, '--post', '--force', origin]` and assert `/dev/status` pid is still the child (never 4400). Add a people-empty + `createEmoji` case that must refuse without `--force`.

4. **P3 — `apps/harmony/seed.mjs:248`**  
   Reset is a second fetch after `holdsData`. A write that lands in the window is erased without `--force`. Irrelevant to an already-full 4400; relevant to an empty store someone just started using.

   **Fix:** Re-read immediately before `/dev/reset`, or send `generation` from `/dev/status` if the door can refuse a stale generation.

5. **P3 — `apps/harmony/README.md:32`**  
   That sentence still says `bun exact.mjs seed` empties the server. The refuse/`--force` rules are four lines later. An agent can take line 32 as the contract and retry with `--force` after the new error (the error itself tells them to).

   **Fix:** On line 32, say seed posts only to an empty server, and that a server that already holds data is refused.

**QUEUE (`QUEUE.md:45`)** — Accurate leftover: conform’s Linux crate path is `apps/<app>/linux` (`conform.mjs` `linuxCrate`), Harmony’s is `apps/harmony/app/linux`; seed times are `Date.now()`-relative; a per-run `--memory` server matching `memory-backend-test.mjs` is the right shape. It does not record findings 1–2.

**Already fail-closed (no issue):** invalid persona / `denied` / non-ok `/q/*` (throw → “cannot tell”; nothing erased); password doors that 403 `/dev/status` (json then query throw, or people still sees public profiles); `seed --force` with no origin (defaults to 4400 by design). Auth and probe errors do not open a reset.

**Not verified:** live `/dev/status.personas` shape (this tree has no `apps/harmony/node_modules/snapback4`). The new test would fail closed if those names were `dev:ada` rather than `ada`.
