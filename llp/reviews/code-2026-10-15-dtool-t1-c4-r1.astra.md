# Code review: LLP 1122 T1 commit 4 (editing and undo), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `3ca4c6160` (commit `1eab66090`, with commit 3's round-1 fixes after it).
- **Method:** one brief (sha256 `5d273747773b55e3738f104135723def78d8eb4ad1e74b4a8c6d56cbd6725d2d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 high, 9 medium).
- **Disposition:** all taken (commit 3 and 4's review fixes, in one commit): a light cell passes over a `when` naming an under-dark axis, as the checker's light side does, and a tie is said, not picked; "only this mode" names every axis at the tuple, and a light column with an under-dark axis offers none; the scope chosen stays with the value typed, either order; Apply writes the proposal shown (its edit and digests carried on it), and is disabled while one is pending or for another draft; a new server epoch empties undo and redo; a write carries every digest the proposal read; a failed undo or redo keeps its entry unless refused for a conflict, which sets it aside, saying who; a value's inputs are read as the checker reads names (a role by its bare name, any case), and a dial input opens its control; a completed write clears only the draft it wrote; refusals show their diagnostics; the real-server tests run through a `--via` proxy that records each write's file once its reply is in and makes the hand edit just before the undo, so nothing is timed, and compare whole files in order, edit the unformatted line itself, redo, and pin through Apply.

---

Reviewed at HEAD, including `3ca4c6160`. No files were changed, and no builds or tests were run.

1. **High — Light cells can edit a dark-only branch.** [app.ts:277](/tmp/rv-dt-4/apps/design-tool/app.ts:277)  
   `mode()` substitutes an under-dark axis’s default value on Light columns. The evaluator instead excludes every `when` naming that axis. After adding `when dark-style=black density=compact`, selecting the compact Light cell therefore targets that dark branch; entering a colour changes Black while Light remains unchanged. **Fix:** exclude branches containing under-dark conditions when selecting a Light cell’s winner, matching `Checker::select(..., true)`.

2. **High — Editing the value cancels “Only this mode.”** [app.contract:215](/tmp/rv-dt-4/apps/design-tool/app.contract:215)  
   `onlyThisMode` sets `editWhen=branch.own`, but the next `cellText` restores `branch.when`. Select a cell showing the default, choose “Only this mode,” enter a value, wait for its proposal, and Apply: the default is edited. Choosing the scope after typing instead discards the typed value. **Fix:** keep scope and value together in a draft; changing either must preserve the other.

3. **High — The review record can differ from the displayed proposal.** [app.ts:297](/tmp/rv-dt-4/apps/design-tool/app.ts:297), [app.contract:401](/tmp/rv-dt-4/apps/design-tool/app.contract:401)  
   Every completed request overwrites global `reviewed`, including replies the resource runtime discards as obsolete. For example, proposal B finishes before A; B remains displayed, but A replaces `reviewed`. Returning the draft to A and pressing Apply while its new proposal loads writes A while displaying B’s diff. Apply does not check `pending(proposal)`. **Fix:** return an immutable review identity with the proposal and apply exactly that proposal’s operations and digests; disable Apply while it is pending or mismatched.

4. **High — Redo history can write into another app.** [app.ts:326](/tmp/rv-dt-4/apps/design-tool/app.ts:326), [app.ts:356](/tmp/rv-dt-4/apps/design-tool/app.ts:356)  
   Reconnection resets recent events but retains both stacks. Undo an edit in app A, then replace its server with app B on port 7740. Redo fetches B’s current digests and sends A’s operation; matching filenames and token names let it succeed. **Fix:** bind history and reviews to the app and server epoch, invalidate them when that identity changes, and reject cross-session replay.

5. **Medium — Default-file writes depend on JSON key order.** [app.ts:304](/tmp/rv-dt-4/apps/design-tool/app.ts:304)  
   With `file=""`, the operation targets the server’s primary design, but the digest comes from the first key in `reviewed.files`. The server’s JSON map sorts keys. An `a.tamber` beside `app.tamber` therefore makes every default-design write carry only `a.tamber`’s digest, producing a false conflict. **Fix:** obtain the primary filename explicitly, or send the returned digest map and record the actual filename from `written`.

6. **Medium — Failed undo and redo requests consume history.** [app.ts:314](/tmp/rv-dt-4/apps/design-tool/app.ts:314), [app.ts:323](/tmp/rv-dt-4/apps/design-tool/app.ts:323)  
   Both operations pop before success. A failed connection during Undo permanently removes its entry; a failed redo transaction similarly loses its redo entry. Even a stale inverse can become valid again if the competing edit is undone—the server retains it until successful use. **Fix:** peek first and move entries only after success; make discarding a blocked entry explicit.

7. **Medium — Text matching misses valid expression dependencies.** [app.ts:287](/tmp/rv-dt-4/apps/design-tool/app.ts:287)  
   `explain.names` contains canonical names, whereas the value retains authored spelling. For the existing valid pattern `label = …; text.shouted = LABEL`, the server reports `-exact-label`; the regex finds nothing and marks the alias as a literal. It can then be overwritten without Pin. **Fix:** expose branch-specific expression/dependency information from the parser, including canonical resolution, rather than inferring it with regexes.

8. **Medium — Dial input links request nonexistent tokens.** [app.contract:360](/tmp/rv-dt-4/apps/design-tool/app.contract:360)  
   Every expression input calls `selectCell`. In the fixture, clicking `accent` or `lift` under `accent.soft` consequently requests `explain` for a dial, which the server refuses with “no token.” **Fix:** distinguish token inputs from dial inputs and route dials to their declared controls, opening their group when necessary.

9. **Medium — “Only this mode” can generate an ambiguous design.** [app.ts:284](/tmp/rv-dt-4/apps/design-tool/app.ts:284)  
   Naming every axis applicable to Light does not prevent ties in Dark. In the supplied fixture, choosing this action on `surface.card`’s compact Light cell adds `when density=compact`, which ties with `when dark-style=dim` in the Dim/compact tuple. The proposal is refused. **Fix:** assess overlaps across all tuples; either propose the necessary intersection branches or explain why this scope cannot be created.

10. **Medium — Completing one write erases a newer draft.** [app.contract:253](/tmp/rv-dt-4/apps/design-tool/app.contract:253), [app.ts:306](/tmp/rv-dt-4/apps/design-tool/app.ts:306)  
    Editing controls remain active during a write. Apply `hue=210`, then prepare another edit before the response arrives: the first response clears global `reviewed`, and `afterWrite` clears the newer `editKind`. The mutation slot serializes writes but does not protect drafts. **Fix:** associate completion with its submitted draft identity and clear only that draft, or disable drafting during the write.

11. **Medium — Proposal refusals discard the actionable diagnostics.** [app.ts:295](/tmp/rv-dt-4/apps/design-tool/app.ts:295)  
    Checked operations return `error.message="refused"` plus detailed diagnostics. The client forwards only the message. An out-of-range dial, a referenced token removal, or the ambiguous mode operation above consequently shows only “refused.” **Fix:** carry and display the diagnostic filename, location, and explanation so the edit can be corrected.

12. **Medium — The real-server test callbacks are not synchronization barriers.** [design-tool.test.mjs:169](/tmp/rv-dt-4/scripts/design-tool.test.mjs:169)  
    The parent polls stdout every 20 ms while the child continues executing operations. It may read `afterDial` after Undo has already run, or perform the hand edit after the intended checkpoint. This makes the gate scheduling-dependent, contrary to `docs/testing.md`. **Fix:** drive one operation at a time through a retained session and complete each assertion or external write before issuing the next operation.

13. **Medium — The assertions do not establish the claimed byte-level gate.** [design-tool.test.mjs:179](/tmp/rv-dt-4/scripts/design-tool.test.mjs:179), [design-tool.test.mjs:232](/tmp/rv-dt-4/scripts/design-tool.test.mjs:232)  
    `changed()` ignores line ordering and duplicate counts, so unrelated reordering can pass the “one-line diff” assertion. The unformatted fixture changes `text.primary`’s whitespace but never edits that token, leaving restoration of an edited unformatted line untested. Successful cell-value undo and real-server redo are also absent. **Fix:** compare complete expected file contents, edit the unformatted line itself, and exercise successful undo/redo against the real server.

The current Stage 3 chip correctly remains “Rebuilds.” Ordinary successful stack operations are LIFO, and redo does fetch current digests; the failures above concern identity, refusal handling, and review integrity.

**Verdict: Request changes before landing.**
