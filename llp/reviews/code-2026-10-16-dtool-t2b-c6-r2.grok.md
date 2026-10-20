# Code review: LLP 1122 T2b step 6 (the inspector panel), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `7ca4d3933`.
- **Method:** one brief (sha256 `4df0993bc15050b616d06c25b64de29e389722e975a58b3940f4df7c3e9c40bf`), shared by both reviewers, blind to the other review, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 high, 1 medium, 1 low).
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

Round 1's other-client inspect limit still stands: the wire has no inspect-state event, and nothing here is worse than that. Edit and Open now open the app design, a present `color_scheme` of `dark` or `light` wins over the build, `selected: null` and a build that stays gone withhold Edit, impact waits for `uses.token`, and Outline names the builds it was told. Three of those fixes do not hold.

## High

**1. A reload makes the old selection current again, and Edit targets that join.** `apps/design-tool/app.ts:540`, `apps/design-tool/app.ts:548`, `apps/design-tool/app.ts:1150`

`staleness` is recomputed from `buildKey` (status revision, scheme, pin revision). It is not stored. A reload re-registers the same id with `status: null`, then reports the same revision, scheme, and pin revision — a page with no pin settles at `[0, "light", 0]` both times (`scripts/tamber-builds.mjs` register sets status null and replays `pin` revision 0; the page reports that revision). Both events are usually in one poll, so the panel never even passes through "moved on". Plan and design digests are dropped in `buildOf`, so a new document with the same key is invisible. The stub test removes `web-2` and leaves it removed (`app.test.contract:832`), so it does not catch the return.

Scenario: Inspect the theme fixture, select the card, reload the canvas. After the new status lands, the pre-reload source, rows, and Edit or Open are shown as current. Apply writes the branch of a node from the document that has gone.

Fix: When a `builds` or `build-status` event makes this build's key, plan, or design disagree with the snapshot, set `selectionNow.stale` and do not clear it until the next `selection`. Compare plan and design, which `shown()` already sends.

**2. An app mode change leaves Edit on the branch that is no longer painted.** `apps/design-tool/app.ts:540`

The same key ignores `status.mode`. `setMode` reports a new mode with the same pin revision (`host/web/dev.js` `facts`). The joined tuple is not updated, and the row stays usable.

Scenario: Pin the theme fixture dark, select the card (Dark, black), then press Dim. The card repaints `dark-style=dim`. The inspector still offers the black column. Apply writes the branch the copy is not showing.

Fix: Put `status.mode` in the key and keep the mismatch sticky, as in finding 1. Reselect is the fresh join.

**3. An older design read rewinds the axes the column is chosen from, so Edit hits the grid's wrong cell.** `apps/design-tool/app.ts:828`, `apps/design-tool/app.ts:889`, `apps/design-tool/app.ts:562`

`designAxes` is assigned for every `describe` that returns, including one whose `number !== issued`. Snapshots, which the grid and `loadBranch` use, are kept only for the winning read. `selectionView` places the column from `designAxes`, and `staleness` treats that cache as the design's axes. A slow read of the previous axes finishes last, the cache matches the selection's snapshot again, and the buttons come back. `designAxes` is also not updated while another `.tamber` file is open, so an axes change there never marks the join stale; `editFromSelection` then sets `file = ""` and the new grid interprets the old column id.

Scenario: The app axes are `dark-style`, then `density`. A selection is placed on `2-dark` (dim, regular). A newer read reorders the axes; an older read completes after it. `2-dark` is now a different tuple. The panel shows Edit, and Apply writes that cell.

Fix: Assign `designAxes` only inside `if (number === issued)`. Place the column from the axes stored on the selection and from the snapshot `loadBranch` will use; if those differ, or the open file is not the app design and its axes are not loaded, show the join stale and render no Edit or Open.

## Medium

**4. After a missed poll, the inspect toggle keeps builds the resync list no longer has.** `apps/design-tool/app.ts:1130`, `apps/design-tool/app.ts:1180`

A `builds` event drops ids that left. A missed gap sends no events, then hello's `takeBuilds` replaces `buildsNow` and does not prune `inspectOn`. A `selected: null` or a leave inside the gap is gone, and a presence lapse turns inspect off on the server without a `selected` event (the page reports that only for a local exit). The selection is marked stale, so Edit is withheld, but `inspectOn` still makes the button "Stop inspecting" and enables Outline. Outline then reports those builds as told.

Scenario: Turn Inspect on, lose the poll long enough that the server drops the client and the page leaves inspect mode, then reconnect on the same epoch with `missed: true`. The tool still says "Stop inspecting". Outline says it sent the sites to `web-1`, and the page draws nothing.

Fix: On `missed`, clear `inspectOn`. After the hello resync, drop any id no longer listed. Outline stays disabled until this client sends `inspect` again and the reply names the builds.

## Low

**5. The README describes the branch as the build's scheme only.** `apps/design-tool/README.md:47`

The panel uses the node's own `color-scheme` when that row is `dark` or `light`, and the build's scheme otherwise. The README says the branch is "its modes and its build's scheme", so a dark subtree on a light build is documented as the light column.

Fix: Say the node's own `color-scheme`, otherwise the build's.

Verdict: Request changes — a reload, a mode change, or a stale design read will still offer the token edit on a branch the selected copy is not showing.
