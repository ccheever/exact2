# Code review: tamber tool --via's dev loop probe tried again on a timeout, round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `70dec984db`.
- **Method:** one brief (sha256 `2abd8f0b228eb58efed16c8bd5d72f91c4cefe4f379bb062bd5274eb10a64979`), shared by both reviewers, each blind to the other's review; commits 6e17fe650f and 70dec984db (round 1's fix). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (no findings).
- **Disposition:** none to take.

Nothing blocks. No findings.

Round 1’s finding is fixed as the record says. The refused-port case in `scripts/tamber-tool-answers.test.mjs` counts fetches and expects one, with five tries allowed. `toolAnswers` calls `fetch` once per attempt, and the wrapper increments before it forwards, so dropping the `TimeoutError` guard (retry every error) makes that count 5 and the assertion fails. A stall no longer fails a single real refusal: the old check was elapsed time under 1 second. The mute server still requires three token probes and at least 5.9 seconds, which is the timeout coverage round 1 asked to keep.

The fetch stand-in is sound, and both the other test in this file and the other glue files keep the original `fetch`.

- It saves `globalThis.fetch`, counts only URLs that start with the closed port’s origin, and forwards every call to that saved function, including the `AbortSignal`. `toolAnswers` passes a string (`${origin}/__tamber/token`), so the prefix matches the probe. `finally` puts the saved function back before `expect(attempts)` and before the mute server in the same test, including when the `expect` throws.
- The other test in the file is the one above it. Bun runs tests in a file one after another unless they are `test.concurrent` or the run passes `--concurrent`. The glue command does neither, and there is no `bunfig.toml` turning concurrency on. That test finishes before this wrapper is installed. Its loop uses another port, and the wrapper only adds a count; it still calls the saved `fetch`.
- The glue step is `bun test --parallel="$workers"` in `scripts/async.mjs`. In Bun 1.4, `--parallel` implies `--isolate`: each file is a fresh global and module registry on a worker, and this command does not pass `--no-isolate`. `docs/testing.md` records that. A replacement of `globalThis.fetch` in this file stays in this file’s global.

The retry in `scripts/tamber-server.mjs` is unchanged by the fix and still matches what round 1 accepted: only `TimeoutError` is tried again, up to `tries`, and both `--via` call sites pass 5 while the `TOOL_ORIGIN` check stays at one.
