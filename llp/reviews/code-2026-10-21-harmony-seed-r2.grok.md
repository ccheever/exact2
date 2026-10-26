# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit, Write and Bash denied, a throwaway worktree at `4910219e88`.
- **Method:** one brief (sha256 `afe7e9520f918cb26e4b257be1041fd2d978cf5f0b1405765ff598d34042a92c`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the seed-guard commits read-only: the diff, the probe replacement, and whether a non-temporary store can still be erased without `--force`. No tests, servers, installs, seeds, or network.Shell is blocked in this session, so I’ll read the seed path, tests, and git metadata directly.Nothing blocks. At `4910219e88`, `bun exact.mjs seed` (that is `seed.mjs --post` with no origin) will not empty a server whose `/dev/status` `temporary` is not strictly `true`. The demo origin stays `http://127.0.0.1:4400`, and the README path does not pass `--force` or `--reset`.

The guard is fail-closed. `devStatus` requires an OK `/dev/status` with a string `storeId` (`apps/harmony/seed.mjs:236-243`). A missing `temporary`, a non-boolean, or anything other than `true` is refused before `POST /dev/reset` (`seed.mjs:255-258`), with a message that seeding would erase everyone's messages and that nothing was erased. `--force` and `--reset` are the only overrides. The same `storeId` and `pid` are required on a second status read immediately before the reset (`seed.mjs:260-263`). Unknown arguments, a second origin, or an origin without `--post` call `process.exit(2)` before `writeFileSync` and before any fetch (`seed.mjs:220-229`). An origin is accepted in any position, so `--post --force <origin>` (how `bun exact.mjs seed --force <origin>` arrives) selects that origin (`app/app.json:44-48`, `scripts/app-cli.mjs:228`).

What the checklist items do:

- A non-temporary store is not erased without `--force` or `--reset`. The only client-side gap left is a swap of the process on that same origin after the second status read and before the reset body is accepted. Snapback's reset body cannot carry the `storeId` just read (it is a seed array, or `{seed, clock}`), so seed cannot close that interval. It is not a path an agent hits by following the README: a persistent 4400 never gets past the first `temporary !== true` check.
- `temporary` is compared with `!== true`, so `"true"`, `1`, an object, or a missing field does not count as in-memory.
- An in-memory server someone is already using is wiped unasked. That is the replacement for the content probe, and the test locks it: a sentinel status is overwritten by a second `--post` with no flag (`scripts/harmony-seed.test.mjs:64-67`). The README says this machine's 4400 keeps its data (`apps/harmony/README.md:36-40`), so that command does not take the unasked path against the demo.
- The README run block is `snapback4 dev --memory --port 4400`, then `bun exact.mjs seed` with no flag. If 4400 is already taken, dev exits with `E_ADDRINUSE` and does not attach. Seed then talks to whatever is already there and refuses unless that process reports `temporary: true`.
- Tests use `--port 0`, never the default origin. The persistent case is a temp copy of `snapback/`, not `apps/harmony`. `afterEach` calls `stopAll` (synchronous kill, start time recorded so a reused pid is not signaled) and then deletes those temp dirs. Sentinels cover `--force` before the origin and `--reset` after it. The bad-argument test only asserts exit code 2. The early return is in the parser, which is synchronous and runs before the write.

No findings.
