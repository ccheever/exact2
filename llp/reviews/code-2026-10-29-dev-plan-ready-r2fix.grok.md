# Code review: dev loops say ready once listening, round 2 fix check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0ff9a86796`.
- **Method:** one brief (sha256 `1bb278321b67b33165679d60987b4ade584e893d4ebc3c6bdf8b647b77681709`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

Nothing is above NIT. No findings.

1. **Early server.** `listening` starts false at `host/web/dev.mjs:519`, and the only place it becomes true is the flush on `host/web/dev.mjs:1499`, after the `[::1]` bind (`1485`), the early banner (`1497`), and `openRemoteCanvas` (`1498`). `sayReady` is the only writer of those lines, so a game's early IPv4 server can no longer print them first.
2. **Taken-port check.** `exited` waits on the child's `close` (`dev-plan-ready.test.mjs:65`), each stream is split on its own, and the unended remainder is checked on `end` (`71`). The assertion is `run.ready() === false` (`118`), after that `close`. On this runtime `end` runs before `close`.
3. **Generation cache.** `afterEach` deletes `target/dev-generations/<sha256>` using `resolveApp` plus `canonicalBytes({ app: app.id, path: app.dir, dist })` (`27–30`), the same inputs as `host/web/dev.mjs:405`. The repo root and the hash inputs match for caltrain, theme-fixture, and canvas-gallery.

Verdict: all three round-2 fixes are correct, and the commit adds nothing above NIT.
