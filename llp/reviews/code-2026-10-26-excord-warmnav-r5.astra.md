# Code review: Excord warm address-following, round 5, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `eecb89e29b`.
- **Method:** a delta brief (not kept), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **MAJOR — [routing.contract:90](apps/excord/routing.contract:90):** Open A → Back to the narrow list → request B → `/nope` before B opens. `openVisits == 0`, so only `supersede()` runs. B’s pending visit remains, but its request is cancelled: the app permanently displays A’s transcript at B’s URL. The pending visit needs to be removed when restoring the list.

- **MINOR — [routing.contract:146](apps/excord/routing.contract:146):** The retained single encoded visit keeps its encoded URL. A subsequent `/nope` calls `stayOpen()`, which pushes a canonical visit because [`go()` matches URL spellings](host/web-js/router.js:100). Back then shows the same channel again. With B pending above the encoded visit, `/nope` appends A above B instead of removing B; Back reaches the cancelled B request.

Both exact round-4 sequences are corrected: [line 87](apps/excord/routing.contract:87) leaves the idle list alone, and [line 143](apps/excord/routing.contract:143) retains one encoded visit whose immediate Back reaches `/`. Ordinary wide navigation and cancellation above a canonical visit remain correct by tracing. Exercised the actual JS router read-only; full E2E execution was unavailable because this worktree has no built app.

REQUEST CHANGES
