# Code review, round 3: setup friction for outside apps (2ec5d985f..71a26dc84), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `c91c6e97d` (the round-2 fixes plus round 2's reviews and dispositions).
- **Method:** brief sha256 `344edac59df6d5275be0eb6d8d60ced8182e96d3f6521b9aaefb211086c2de77`, the same one sent to grok; blind to grok's round 3. The last round (RULES: three).
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** fixed in `aacdcd75e`: `game/new-app.test.mjs` gains a deterministic case, an empty `CARGO_HOME` with `CARGO_NET_OFFLINE=true`, in which `createApp` reports the deferral and leaves exact2's lock byte for byte. The restore after a Cargo failure is unconditional, so a failure before and one after Cargo writes the lock take the same path; no test can make Cargo stop between the two. The older scaffold test builds and drives the app, so it needs a warm Cargo cache either way; before this change it failed at `createApp` on a cold one.

---

Round-2 dispositions:

- **Astra #1:** accept the runtime fix: deferred creation restores the copied lock even after Cargo rewrites it. The tests remain incomplete, below.
- **Astra #2 / Grok #2:** accept: embed, archive and install completion messages report the Metal skip; the unreachable macOS clause is removed.
- **Astra #3 / Grok #1:** accept: explicit engine overrides disable cache selection consistently; a selected cache requires a receipt and the pinned commit.
- **Astra #4:** accept: Rolldown loads after preflight. The actual entrypoint without `node_modules` now reports `bun install --frozen-lockfile`.
- **Grok #3:** accept: the comment correctly identifies deferred scaffolding as the lock-mutation exception.

No new production-code regression found in the round-3 fixes. One full-change issue remains:

1. **Should-fix — [game/new-app.test.mjs:51](/tmp/rv-setup3/game/new-app.test.mjs:51): the earlier scaffold test still fails with a cold Cargo cache.** Its initial `createApp(dir)` now succeeds by deferring resolution, but it subsequently calls `createApp(dir, { update: true })` here and at line 57. `updateApp` calls non-deferrable `resolveOffline(dir)` at [game/new.mjs:326](/tmp/rv-setup3/game/new.mjs:326), so the same missing crate makes the test throw. The conditional added to the separate sibling-app test does not cover this path. Keep update assertions on a resolved fixture, and add deterministic deferred-creation cases for Cargo failures both before and after rewriting the lock, rather than relying on the machine’s cache.

Read-only probes confirmed lock restoration and optional/required Metal behavior. No compilation or full suites ran; `git diff --check` passed and the worktree remains clean.

LAND WITH FIXES
