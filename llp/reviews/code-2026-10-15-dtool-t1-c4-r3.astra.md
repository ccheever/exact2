# Code review: LLP 1122 T1 commit 4 (editing and undo), round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `be00071d7` (commit `d3e6ad575` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `57a1a2022b292a9d8308e94aac1413dfd89fdcd0d6748a899ca3b6914a6b24e9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 4 medium, 2 low).
- **Disposition:** taken but 6: a write, an undo, a redo and an accept read `session` before their first await and change neither stack if it moved; each goes with the token of the server run its review, its inverse or its listing came from (tokens are per run), never another's, so a server now on the port refuses it as unknown, which the tool reads as a restart and empties the stacks (a bare not-found; an operation's own 404, such as a proposal gone, keeps its words); a cell's draft whose branch the grid no longer shows is not applied, said; the panels show only the read of the file chosen; a write's refusal newer than the last poll is shown. Not done (round 3 was the last): 6, an undo whose reply the wire gives up on at 120 s while it later lands still loses its redo entry (it is set aside, said); that needs the wire to hold the request until the server answers.

---

Read-only review; no files changed and no builds or tests run.

1. **High — Redo can survive a reset during its preflight.** [app.ts:654](/tmp/rv-dt-4r3/apps/design-tool/app.ts:654)  
   `top` is captured before `sameRun()`, but `session` is captured afterward. Suppose a background read has obtained B’s token, Redo captures A’s entry, and B’s events reply calls `forget(B)` while B’s hello is pending. `sameRun()` then compares B against the newly updated `minted`, succeeds, and sends A’s retained operations with B’s current digests. Clearing the arrays does not invalidate `top`. **Fix:** capture the entry’s epoch and session before the first await, and check them again before sending and settling the mutation.

2. **High — Disabling retries does not bind a write to the token its preflight used.** [app.ts:59](/tmp/rv-dt-4r3/apps/design-tool/app.ts:59), [app.ts:614](/tmp/rv-dt-4r3/apps/design-tool/app.ts:614)  
   `renew=false` only prevents retrying that request’s own 404. Its first attempt still takes the current global token. A delayed successful hello from A can finish after another read has obtained B’s token, but before B’s poll resets the session. The subsequent transaction authenticates to B; matching file digests let it write there. **Fix:** carry the exact authenticated token, epoch, and generation from preflight into the mutation. Bind proposal receipts to their request’s identity too, rather than stamping `place.epoch` at completion. The restart tests keep a fixed token and do not exercise this interleaving.

3. **Medium — Accept can repopulate undo with an inverse from a previous server run.** [app.ts:762](/tmp/rv-dt-4r3/apps/design-tool/app.ts:762)  
   Unlike write, undo, and redo, `acceptHeld()` performs no session check after its request returns. If A accepts a proposal, B’s poll clears the session, and A’s delayed response then arrives, it pushes A’s inverse onto B’s undo stack and reports it with B’s current session number. Undo subsequently fails because B never issued that inverse. **Fix:** check the captured generation before and after the entire accept operation, before modifying either stack.

4. **Medium — Drafts remain attached to branches that disappeared.** [app.contract:278](/tmp/rv-dt-4r3/apps/design-tool/app.contract:278), [app.ts:595](/tmp/rv-dt-4r3/apps/design-tool/app.ts:595)  
   The panel now checks its revision, but the draft carries no originating revision. Draft an edit to `surface.page`’s Dim branch, then let another client remove that `when`. The event automatically reproposes the retained `editWhen` against fresh digests. The refreshed panel says “default,” yet Apply recreates Dim without “Only this mode.” **Fix:** bind the draft to its original file, revision, and branch; require an explicit new selection or scope choice when that branch disappears.

5. **Medium — Switching files leaves the previous file’s dials editable while loading.** [app.contract:503](/tmp/rv-dt-4r3/apps/design-tool/app.contract:503)  
   `chooseFile` clears the existing draft, but the retained `design` remains rendered without checking `design.file == file`. Switch from `app.tamber` to `accent.tamber` and adjust a dial before the new read completes: the control still shows the first file’s value and metadata, while the new proposal targets the second file. **Fix:** disable editing until the displayed design belongs to the selected file, and bind edit callbacks to that read’s identity.

6. **Medium — A successful Undo whose response is lost still loses its redo entry.** [app.ts:636](/tmp/rv-dt-4r3/apps/design-tool/app.ts:636), [tamber-server.mjs:213](/tmp/rv-dt-4r3/scripts/tamber-server.mjs:213)  
   The 130-second client timeout does not solve this: the wire returns 504 after 120 seconds while retaining the callback for eventual completion. If Undo subsequently succeeds, its inverse is consumed. Retrying now pops the entry on “no inverse” without placing it on redo. The permanent blockage is fixed, but history is still lost. **Fix:** make mutation outcomes recoverable or idempotent, so a retry can distinguish an already completed undo from an expired inverse and update both stacks correctly.

7. **Low — An ordinary proposal 404 is misreported as a server restart.** [app.ts:64](/tmp/rv-dt-4r3/apps/design-tool/app.ts:64)  
   Accepting a proposal that another client accepted or dropped, or that expired, legitimately returns 404 from [tamber-server.mjs:482](/tmp/rv-dt-4r3/scripts/tamber-server.mjs:482). The new handling discards that explanation, clears authentication, and claims the server restarted and history reset. **Fix:** distinguish authentication’s not-found response from structured operation errors and preserve the latter.

8. **Low — Preflight restart refusals can be hidden by the stack display.** [app.contract:296](/tmp/rv-dt-4r3/apps/design-tool/app.contract:296)  
   When `sameRun()` resets history before the next poll, its failed mutation has a newer `session` than `changes`. The derive treats any inequality as an obsolete mutation and substitutes a successful result, hiding the refusal. The subsequent poll can increment the session again, keeping it hidden permanently. **Fix:** distinguish newer from older generations and preserve the mutation’s error independently of stack-depth synchronization.

Verdict: **Request changes — cross-server writes remain possible, and revision and undo recovery fixes are incomplete.**
