# Code review: the web realms' load-time listen door (x2/web-listen-door), quick review, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `605d7be8e5`.
- **Method:** one brief (sha256 `ce03c2a7730986b6fe63fb97da8cdd7d0dbb0246d8b0ae41592dcc2af9266c1c`), one reviewer; a quick review of a mechanical main-red fix (the lead's 15:41 ruling: a quick review plus a narrow re-check). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** One P2 (fake-clock's prelude realm had no door): fixed in the next commit.

1. **P2 — One test scaffold still misses the load-time probe.** [fake-clock.test.mjs:124](host/web/tests/fake-clock.test.mjs:124) evaluates the full prelude without installing `__exact_host`. The `listen` probe therefore throws `TypeError: host is not a function`, failing the expected `'loaded'` assertion when Chrome is available. It needs a minimal door answering that probe before evaluation. This is an existing regression left unfixed, not introduced by this commit.

The three-file diff itself is correct and minimal:

- Both production doors and both worker stubs handle only the `listen` probe before the context guard.
- `''` correctly fails the prelude’s `=== "1"` check, leaving `exactListen` absent.
- Native’s door already handles the probe; `host/web-js` uses separate facades without this prelude call.
- The `address-space.js` copies/rewrites and `drive`/`Origin` exports match current dependencies. Assertions are unchanged; no additional production behavior changes.

Static review only; no edits, builds, or tests run.
