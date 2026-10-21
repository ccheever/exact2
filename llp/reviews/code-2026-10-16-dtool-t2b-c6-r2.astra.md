# Code review: LLP 1122 T2b step 6 (the inspector panel), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `7ca4d3933`.
- **Method:** one brief (sha256 `4df0993bc15050b616d06c25b64de29e389722e975a58b3940f4df7c3e9c40bf`), shared by both reviewers, blind to the other review, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 4 medium, 1 low).
- **Disposition:** taken, but Astra 6 and Grok 4 in part.
  - A selection once stale stays stale until a new selection. Each `builds`, `build-status` and hello resync is checked as it comes, so a reload that comes back to the same facts in one answer still ends it. The build facts it was made under now include the status mode, plan and design. An `unsupported inspect` for its build ends it too.
  - The app design's axes come only from the newest read: `describe` for the open app file, or `loadAppAxes` while another file is open.
  - The inspector's Edit and Open pass the modes of their column (its label). A branch read since on another tuple at that column id is refused, with "select it again", instead of being shown.
  - A trimmed report with no `color-scheme` row leaves the scheme unknown.
  - `inspectSent` refreshes the inspector, so the toggle follows its own command at once.
  - A build that leaves keeps its place in the toggle, since the server keeps its inspect mode and replays it. A gap (`missed`) clears the toggle. Turning inspect off clears it for every build.
  - Rows use the author's CSS names (contract/lower's style_names.rs): `-exact-` and `-webkit-` prefixes, `position`, and a border side's longhand with the side first.
  - The README says the scheme is the node's own `color-scheme`, else its build's, and when a selection stops being current.
  - The selection's view moved into apps/design-tool/inspector.ts.
  - New tests: a reload back to the same facts, a mode the app moves, a trimmed report, a cell opened after the axes were reordered (another file open, so the app design is read again), the toggle without a poll, and the border longhand's name.
  - In part: the stub has no dark-subtree branch, so the exact branch label is still asserted only on the wasm drive (`default (Light)`). A build that left still counts for Outline, whose answer names the builds actually told.
- **Also:** on the rebased main, the T1 test "an undo goes only to the server run that issued it" failed (main red). It is fixed in its own commit before this one.

---

Read-only review of `7ca4d3933`; no files changed or tests run. The round 1 fixes remain incomplete.

1. **High — A reloaded selection becomes current again without being selected again.** [app.ts:540](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:540), [app.ts:1158](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:1158)  
   `buildKey` contains only pin revisions and scheme; it omits plan, design and modes. Disappearance and re-registration produce only a temporary staleness comparison, rather than permanently invalidating the join. Reload a selected tab: once its original scheme and pin revision return, the old Edit/Open buttons return too, although the page has no selection. A changed plan likewise goes unnoticed, and an `unsupported inspect` report saying the node vanished only updates the build diagnostic.  
   **Fix:** retain the build’s identity and effective status, and latch invalidation while folding disappearance, re-registration, generation changes and inspect failures. Only a fresh joined selection should clear it.

2. **High — Switching to the app’s design can still open the wrong branch.** [app.ts:1164](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:1164), [app.contract:607](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.contract:607)  
   A selection snapshots the cached `designAxes`, which update only when `loadDesign` reads the app’s file. While another file is open, `loadAppAxes` reads fresh app axes without updating that cache. For example, reorder `[dark-style, density]` to `[density, dark-style]`, then select Dim/Regular. The inspector still passes old column `2-dark`; opening the app’s refreshed design interprets it as Black/Compact. The later stale label does not clear the cell already opened. Also, [app.ts:828](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:828) still updates this cache before the overtaken-read guard.  
   **Fix:** carry named modes and scheme into the action, then resolve them against the accepted app-design snapshot used by `loadBranch`. Reject revision mismatches; never carry positional column IDs between snapshots.

3. **Medium — A trimmed report can silently choose the build’s scheme instead of the node’s.** [app.ts:1162](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:1162)  
   The page trims oversized reports by removing trailing rows ([dev.js:475](/tmp/rv-dt-t2b6-r2/host/web/dev.js:475)). If that removes `color_scheme` while retaining an editable colour row, this fallback treats the root scheme as known. A dark Dim subtree in a light build then offers the Light/default branch again. `trimmed` produces only an informational message.  
   **Fix:** preserve effective scheme outside the trimmable rows, or withhold branch actions when a trimmed report lacks it. Missing scheme evidence is different from an explicit `normal`.

4. **Medium — Successful Inspect commands do not refresh the toggle.** [app.contract:443](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.contract:443), [app.ts:1421](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:1421)  
   `sendInspect` changes `inspectOn`, but `inspectSent` neither refreshes `inspected` nor participates in its arguments. The server emits no event for an ordinary inspect command. On an otherwise idle build, the banner appears while the button continues saying “Inspect” until the next poll completes, potentially 25 seconds later. Pressing it again sends another start. Stopping has the reverse problem, including Outline remaining enabled.  
   **Fix:** refresh `inspected` when the mutation settles, or expose the acknowledged state through a reactive dependency. This occurs with this client alone, beyond the accepted cross-client limitation.

5. **Medium — This client’s own build reconnect loses the toggle state permanently.** [app.ts:1150](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:1150)  
   A disconnect deletes the build from `inspectOn`. The server retains inspect ownership across a build disconnect and replays `on: true` when that same build registers again ([tamber-builds.mjs:224](/tmp/rv-dt-t2b6-r2/scripts/tamber-builds.mjs:224)). Nothing restores the tool’s entry. Consequently, reconnecting the sole inspected tab leaves its banner active while the tool says “Inspect” and disables Outline. No other client is involved.  
   **Fix:** track ownership separately from connected recipients and reconcile reconnects, or expose explicit start/stop controls when state cannot be established.

6. **Medium — The revised tests still miss the critical regression cases.** [app.test.contract:793](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.test.contract:793), [app.test.contract:827](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.test.contract:827), [design-tool.test.mjs:940](/tmp/rv-dt-t2b6-r2/scripts/design-tool.test.mjs:940)  
   The Dark test asserts only `selColumn`, not the opened branch or write scope. The departure test removes a selection already non-editable because its scheme is unknown. The real drive verifies only the Light/default branch and selects a node before stopping, supplying the event that masks finding 4. The stub’s immediate polls also hide that delay. These tests pass with the reload and axis bugs above.  
   **Fix:** exercise an initially editable selection through reload, test axis changes while another file is open, assert the dark subtree’s exact branch and edit scope, and check toggle completion with an idle long poll.

7. **Low — The CSS-name conversion remains incomplete.** [app.ts:539](/tmp/rv-dt-t2b6-r2/apps/design-tool/app.ts:539)  
   Replacing underscores produces names such as `border-color-top` and `border-radius-top-left`; authors write `border-top-color` and `border-top-left-radius`. It also loses prefixes from names such as `-exact-mode`. Selecting a bordered card exposes these incorrect labels.  
   **Fix:** use the canonical vocabulary mapping in [style_names.rs:160](/tmp/rv-dt-t2b6-r2/contract/lower/src/style_names.rs:160), including reordered longhands and prefixes.

The matching-token impact guard, Outline’s named recipients, explicit `selected: null` invalidation, observed epoch reset and full call-site paths address their earlier findings.

**Verdict: Request changes — selection validity and branch targeting still do not reliably fail closed.**