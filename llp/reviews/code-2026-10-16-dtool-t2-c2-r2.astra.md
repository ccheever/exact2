# Code review: LLP 1122 T2 step 2 (builds panel and pins), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `dae314424` (commit `39d166329` and its round-1 fixes).
- **Method:** one brief (sha256 `795d8e78afa42e9260eeb86e7ed80c4a28415b2618ff2e452aab959145b70780`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium, 2 low).
- **Disposition:** taken but 6 (grok) in part: retyping the name keeps the tool's client (it polls as the saved name until the new one is saved, and says a new name is a new client whose pins are handed back); a poll a newer one replaced changes nothing when it answers, and a failed hello read during a resync keeps the list and is read again each poll until one succeeds; a build's last refusal is now on its server record (`unsupported`, tamber/1.3, scripts/tamber-builds.mjs): the pin's that the build reports next, kept while its status stays at that revision and gone once it moves on, so a delayed refusal is not pinned on a later pin, and hello and both build events carry it; the pin form's modes are the app's own design's axes whichever file is open, and the per-row preview is gone: Preview in the form sends one axis to one build, enabled only for exactly that; a root font size outside 4-64 is not sent and keeps Pin off; the stub replaces build records instead of editing them, and keeps a refusal on its record; the real drive keeps the tool open over 36 s after the pin with no hand-back, and the hand-back after it closes lands between 10 and 45 s. 6 in part: a hand-back to a build that has not reported since it reloaded shows "Not reported yet" rather than "handing back" (its record cannot tell a hand-back from no pin).

---

Read-only review completed; no files changed or tests run. Four Medium and two Low findings:

1. **Medium — Failed resynchronization permanently loses builds.** [apps/design-tool/app.ts:940](/tmp/rv-dt-t2b-r2/apps/design-tool/app.ts:940)  
   If `hello` fails after a missed poll, the code empties `buildsNow`, advances the cursor, and returns success. Subsequent polls never retry the snapshot. Quiet, connected builds can remain absent indefinitely. **Fix:** retain a resynchronization-needed flag until `hello` succeeds, report the failure, and retry without treating the empty list as authoritative.

2. **Medium — Superseded polls can overwrite newer build state.** [apps/design-tool/app.ts:896](/tmp/rv-dt-t2b-r2/apps/design-tool/app.ts:896)  
   Changing the person’s name changes the resource’s `client` argument and starts another poll. The old promise continues mutating module state even though the runtime discards its eventual resource answer. A delayed disconnect response can overwrite a newer reconnect; the next empty poll publishes that obsolete list. **Fix:** guard reconciliation with a request generation, including after the awaited `hello`, so superseded calls cannot mutate shared state.

3. **Medium — Delayed refusals attach to the wrong pin.** [apps/design-tool/app.ts:935](/tmp/rv-dt-t2b-r2/apps/design-tool/app.ts:935)  
   `unsupported` is assigned the build’s *current* `pinRevision`, although the report carries no revision. Pin contrast at revision 1, then preview an axis at revision 2 before the build answers: a delayed `pin.contrast` refusal becomes revision 2’s diagnostic and survives its successful status. **Fix:** carry the answered pin revision through host reports and server events; associate diagnostics with that revision.

4. **Medium — Pin choices follow the selected file instead of the app’s axes.** [apps/design-tool/app.contract:736](/tmp/rv-dt-t2b-r2/apps/design-tool/app.contract:736)  
   `design.axes` belongs to whichever file the editor selected. Selecting an axis-free `brand.tamber` removes the running app’s mode controls; selecting a different design can offer modes the build cannot apply. **Fix:** load and refresh the app’s axes independently of the selected editor file, and use them for both pin and preview controls.

5. **Low — The lifecycle test permits missing the promised deadline.** [scripts/design-tool.test.mjs:763](/tmp/rv-dt-t2b-r2/scripts/design-tool.test.mjs:763)  
   A hand-back at 49 seconds passes despite the 35-second requirement. The server’s 35-second lease plus its five-second expiry sweep can actually retain pins for nearly 40 seconds after closure. **Fix:** expire at the lease deadline and assert the stated bound with a small scheduling allowance.

6. **Low — Stub pinning rewrites earlier events.** [apps/design-tool/stub/tamber-stub.ts:63](/tmp/rv-dt-t2b-r2/apps/design-tool/stub/tamber-stub.ts:63)  
   `/__stub/event` shares build objects between `BUILDS` and the event log. Pinning mutates those objects, so an earlier registration event can retroactively contain a later pin and revision. The real server produces separate snapshots. **Fix:** replace build records immutably or clone injected snapshots, and verify that later pins leave earlier events unchanged.

Verdict: Request changes for synchronization, refusal attribution, and app-axis selection.
