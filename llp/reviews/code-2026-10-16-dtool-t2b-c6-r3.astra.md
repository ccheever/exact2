# Code review: LLP 1122 T2b step 6 (the inspector panel), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `3af389f4d`.
- **Method:** one brief (sha256 `30bf48a7e6f7dcad994d26fb739573ae7659b1af8638092441561516502d8f1f`), shared by both reviewers, blind to the other review, with rounds 1 and 2's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 3 medium).
- **Disposition:** taken, but Astra 3 and 5 in part. This is the last round.
  - The build facts a selection was made under now read the build's own `plan` and `design` (the server keeps them beside `status`, not in it). A rebuild or a live batch therefore ends the selection, with a test using the wire's shape.
  - A cell the inspector opens carries its column's place, whole: the scheme and every axis's value by name, not the label, which leaves defaults out. A branch read since on another place is refused, a changed default included.
  - With no app axes read yet, or a copy naming an axis value the design does not list, the copy is not placed, said as such ("the app's design is not read yet", "its modes are not a column of the app's design"), and nothing is offered.
  - Reads of the app design's axes (the open design's, or `loadAppAxes` while another file is open) share one counter, so only the newest sets them.
  - Server (scripts/tamber-builds.mjs, docs/tamber-protocol.md): `inspect` off with no build also clears the client's inspect mode on its builds that are away, so one that registers again is told off, not on. Tested in tamber-builds.test.mjs.
  - In part (Astra 3): an accepted `loadAppAxes` does not itself refresh the inspector. A stale view's Edit is still refused at the cell by its place.
  - In part (Astra 5): a presence lapse with no event gap is not seen by the tool. The server turns inspect mode off and publishes nothing, and data sources have no clock to measure the gap. The toggle can say "Stop inspecting" until the next command or a gap; a stale selection still offers nothing.

---

Read-only review of `3af389f4d`; no files changed or tests run.

1. **High — Plan/design changes still leave old selections editable.** [inspector.ts:30](/tmp/rv-dt-t2b6-r3/apps/design-tool/inspector.ts:30)  
   `buildKey` reads `status.plan` and `status.design`, but the server puts those fields on the build record itself; `buildOf` also discards them. Both key entries therefore remain null. A wasm generation swap that changes a node’s token from `surface.card` to `accent.fill`, while preserving scheme, modes and pin revision, leaves the old “Edit surface.card” current.  
   **Fix:** retain and compare the build’s top-level digests. Add a regression using the actual wire shape with changed digests and otherwise identical status; the new re-registration test only exercises `status: null`.

2. **High — Display-label equality still permits editing the wrong branch.** [app.contract:1051](/tmp/rv-dt-t2b6-r3/apps/design-tool/app.contract:1051)  
   `selModes` and `branch.modes` are column labels, which omit default axis values. Select `surface.inset` at Light/Regular and open its editor, then change `density = regular | compact` to `compact | regular`. Column `0-light` still says “Light”, but now selects `when density=compact`. The guard passes and the editor permits writing Compact, even after the inspector marks the original selection stale.  
   **Fix:** carry the complete named tuple, scheme and axis revision into the branch lookup. Compare those identities, not labels. Test a changed default; the existing axis-order test uses a nondefault label and misses this case.

3. **Medium — An overtaken app-axis read can overwrite the newer design’s axes.** [app.ts:1383](/tmp/rv-dt-t2b6-r3/apps/design-tool/app.ts:1383)  
   `appReads` is independent of `loadDesign`’s counter, and returning to `file = ""` does not increment it. A delayed `loadAppAxes("brand.tamber")` response can therefore overwrite axes already accepted by a newer app-design read. A fresh selection made against that newer design subsequently becomes permanently stale against the obsolete cache. Also, accepting `appAxes` does not itself refresh `inspected`.  
   **Fix:** invalidate pending axis reads on every file switch, coordinate both writers to `designAxes`, and make accepted app-axis changes refresh and invalidate the selection immediately.

4. **Medium — Stop forgets disconnected builds without stopping their retained inspect mode.** [app.ts:1397](/tmp/rv-dt-t2b6-r3/apps/design-tool/app.ts:1397)  
   With one client: start inspection, disconnect the build, press Stop, then reconnect it. The server’s inspect command targets only connected builds, so it retains the disconnected build’s ownership. This code nevertheless clears every local ID. On reconnect the server replays inspection on, while the tool says “Inspect” and disables Outline.  
   **Fix:** clear retained ownership on the server, or retain pending stop requests and deliver them on reconnect. The stub only returns recipient lists, so it cannot establish this ownership guarantee.

5. **Medium — Presence expiry need not produce `missed: true`.** [app.ts:1101](/tmp/rv-dt-t2b6-r3/apps/design-tool/app.ts:1101)  
   Suspend polling beyond the presence lease while inspecting, with no pins or other activity. The server turns inspection off without publishing an event. Resuming on the same epoch returns `missed: false`: that flag detects missing buffered events, not expired presence. The tool therefore keeps “Stop inspecting”, enables Outline, and can retain the selection as current indefinitely. This occurs with one client.  
   **Fix:** track presence continuity separately, invalidate selection/inspect state when that continuity is lost, and explicitly re-establish inspection. Test lease expiry without an event-buffer gap.

**Verdict: Request changes — branch targeting and selection/inspect freshness remain incorrect.**