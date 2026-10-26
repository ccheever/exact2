# Code review: Excord warm address-following, round 6, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `c72dff7b95`.
- **Method:** a delta brief (not kept), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**MAJOR — [apps/excord/routing.contract:142](apps/excord/routing.contract:142): encoded return over a pending request retains duplicate history.** Open A at its canonical address, request B, then navigate to encoded A before B opens. The stack becomes `[/, A, B, encoded-A]`. `takeDecoded` cancels the request but sees B immediately below, so it retains all four visits. Back consequently requests and opens B ([line 119](apps/excord/routing.contract:119)). Baseline’s `openVisits > 1` returned to the original A visit. HEAD introduces this regression; the check must account for an existing A farther down.

The other requested traces check out in source/router replay:

- In-flight `/nope`, including after the narrow list: navigation and request remain unchanged; B completes at B’s URL ([routing:89](apps/excord/routing.contract:89), [updated assertion](apps/excord/fixture/routing.e2e.test.ts:246)).
- Encoded A immediately above A collapses correctly; a single encoded A survives `/nope` and `stayOpen` without duplication.
- Following into open A with one pending visit above it pops that visit; wide `/` uses the same behavior ([routing:73](apps/excord/routing.contract:73), [routing:91](apps/excord/routing.contract:91)).

Validation used the real JS router with transcribed action branches. All 12 e2e tests skipped because this worktree has no Excord build. Worktree unchanged.

REQUEST CHANGES
