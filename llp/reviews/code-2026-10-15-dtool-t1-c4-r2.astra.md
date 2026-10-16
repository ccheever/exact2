# Code review: LLP 1122 T1 commit 4 (editing and undo), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `540fe0e2a` (commit `d3e6ad575` and its round-1 fixes).
- **Method:** one brief (sha256 `065c7291ec4c92997f8571343a4d06b75051b16744c7c6e4597d4f71feb3b1ca`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 5 medium, 2 low).
- **Disposition:** all taken: hello carries the server's epoch; a write, an undo, a redo and an accept first check the server is the run the session's stacks are of, else empty them and refuse; a write is not sent on with a renewed token, and one whose session reset mid-request is refused; a Branch names the design read it is of, its panel shows only for that read when not pending, and an `explain` of another revision is read again with the grid; choosing another file clears the draft, selection, pin and scope; platform roles and system colours a value names are inputs (so overwriting one is a pin), and a value is read as whole words (no word inside a hex colour); Cancel clears "only this mode" and the heading names the branch Apply writes; writes wait 130 s, longer than the server's own, and an inverse the server no longer has is set aside, said; a dial with `control=none` is said, not offered; the pin test also applies a pin the checker takes and undoes it to the bytes.

---

Reviewed `d3e6ad575` with fixes at `540fe0e2a`. No files changed; no builds or tests run.

The light-branch selector, tie handling, scope/value preservation, carried proposal digests, newer-draft completion check, diagnostics, no-op redo handling, and ordered file comparisons are corrected. The following problems remain.

1. **High — Redo can still write into another server’s app.** [app.ts:454](/tmp/rv-dt-4r2/apps/design-tool/app.ts:454)  
   History resets only when `waitChanges` observes the new epoch. Undo in app A, replace its server with app B, then click Redo before that poll completes: `call` refreshes authentication, `hello` supplies B’s digests, and A’s operation writes into B. Clearing the stacks during the request also leaves its captured `top` usable. **Fix:** bind entries, reviews, and in-flight mutations to the authenticated server instance; reject a changed instance before sending or settling a mutation, and prevent writes from transparently retrying across authentication changes.

2. **Medium — A refreshed grid can still edit its previous branch.** [app.contract:405](/tmp/rv-dt-4r2/apps/design-tool/app.contract:405)  
   The panel checks token and column, but neither the design revision nor `pending(branch)`. If another client removes the selected Dim `when`, the refreshed grid shows the default while the pending branch resource retains Dim. Typing during that window captures the removed `when`; Apply recreates it without “Only this mode.” **Fix:** carry file/revision identity on Branch, disable editing until it matches the displayed design, and invalidate drafts whose target branch disappeared. The separate `explain` read also needs a matching digest.

3. **Medium — Changing files silently moves an existing draft.** [app.contract:313](/tmp/rv-dt-4r2/apps/design-tool/app.contract:313)  
   `chooseFile` changes only `file`. A token edit drafted in `app.tamber` is consequently reproposed against `accent.tamber`, retaining its target, value, and `when`. With matching token names, Apply changes the second file; its selected cell may show a different branch. **Fix:** bind the draft to its originating file, or clear the draft, selection, pin, and scope when navigating to another file.

4. **Medium — Expression detection still permits implicit pinning.** [app.ts:400](/tmp/rv-dt-4r2/apps/design-tool/app.ts:400)  
   `expression` means “found a declared token or dial.” The existing `text.system = -exact-secondary-label` fixture has empty `names` and `dials`, so it gets a directly editable field. Replacing it with a literal removes its platform dependency without Pin. The regex also misreads `#1abcde` as containing `abcde` if that token appears in another branch’s dependency list. **Fix:** obtain branch-specific expression and dependency information from the parser, including platform roles, rather than infer it from these lists and text matching.

5. **Medium — Cancel retains “Only this mode.”** [app.contract:286](/tmp/rv-dt-4r2/apps/design-tool/app.contract:286)  
   Cancel clears `editKind` but leaves `scopeOwn=true`. Select a default-backed dark cell, choose “Only this mode,” cancel, then enter another value: the next edit creates the mode-specific `when`, although the panel still labels the branch “default.” **Fix:** reset the scope when cancelling and restore the editor to the displayed branch.

6. **Medium — Losing a successful Undo response permanently blocks history.** [app.ts:442](/tmp/rv-dt-4r2/apps/design-tool/app.ts:442)  
   Retaining entries fixes failures before execution, but not an Undo that executes before its response is lost. The server consumes that inverse on success ([tamber_serve.rs:759](/tmp/rv-dt-4r2/contract/cli/src/tamber_serve.rs:759)). Retrying returns “no inverse,” which lacks `conflict`, so the unusable entry remains forever, blocks earlier undo entries, and never reaches redo. **Fix:** make mutation outcomes recoverable/idempotent and distinguish permanent inverse refusals from retryable transport failures.

7. **Low — Hidden dial inputs still lead nowhere.** [app.ts:401](/tmp/rv-dt-4r2/apps/design-tool/app.ts:401)  
   The fixture’s `tone.deep` offers `shade`, but `shade` declares `control=none` and is absent from `snap.dials`. The fallback creates an actionable input anyway; clicking it only highlights a nonexistent control. **Fix:** represent unavailable controls explicitly and show an explanation instead of an action.

8. **Low — The pin test still proves only refusal.** [design-tool.test.mjs:308](/tmp/rv-dt-4r2/scripts/design-tool.test.mjs:308)  
   The revised test checks `tamber-dial-inert` and cancels. It never applies a successful pin, despite the fix’s claim of testing pin through Apply. **Fix:** also pin a valid candidate such as `accent.fill`, whose dial remains used elsewhere, assert the complete resulting file, and undo to the original bytes.

**Verdict: Request changes.**
