# Code review: the design tool's undo stacks after a server restart (main red), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `8af03e0de`.
- **Method:** one brief (sha256 `2dbceab886a7c571429da6b4933a63ef78104c5aff2f56ec758bbd045d369761`), a quick review under the main-red standing permission. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** needs changes (1 medium).
- **Disposition:** taken. `forget` drops entries by the token of the run that issued them, not by epoch. A refused write drops the entries of the run its token named, and the poll drops every entry not of the run it now reads. The session moves on only when something was dropped. That also fixes the limitation noted in passing: a write made to a new run before the poll sees it keeps its undo.

---

- **Medium — [apps/design-tool/app.ts:490](/tmp/rv-dt-forget/apps/design-tool/app.ts:490), guard at [line 1083](/tmp/rv-dt-forget/apps/design-tool/app.ts:1083): `forgotFor` can identify the wrong server run.** Scenario: the poll advances A→B, then an outstanding A-bound undo returns `RESTARTED`. Its unconditional `forget()` records **B**, because that is now `place.epoch`. A subsequent B write creates an undo entry; when B→C occurs, the poll skips `forget`, retaining B’s invalid inverse and omitting the session bump. The guard also fails if A’s refusal clears the stacks, a B write succeeds, and C starts before any poll observes B. **Fix:** track the server token owning the stacks and pass the failed request’s token into invalidation. Deduplicate against that identity and compare it with the poll’s `used` token, rather than mutable `place.epoch`.

I traced every caller: write, scrub release, undo, both redo stages, both commit stages, proposal acceptance, import editing, and the poll. For the intended refusal-first ordering:

- The refusal’s answer survives, and a subsequent B write retains its undo when B’s poll arrives.
- Once the poll has observed B, an ordinary B→C restart correctly forgets.
- Empty-epoch initialization introduces no new regression.
- `commitSession` still supplies the session increment through `forget`; the next poll exposes it. Other restart cleanup remains unconditional.

A pre-existing limitation remains: a successful fresh-token write **without an earlier restart refusal** still loses its undo when the new epoch is polled.

Source review only; no tests run or files changed.

Verdict: Needs changes—the reported ordering is fixed, but genuine later restarts can retain invalid undo state.