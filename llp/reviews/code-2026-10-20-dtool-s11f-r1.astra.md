# Code review: the design tool's half of LLP 1121 stage 11 D on the real server (the stub dropped), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `60bf87b1d8`.
- **Method:** one brief (sha256 `b07cd878e977efe5f68102c7bcee6cf4ea4835fa9d2f0e6745d2a8f51d823de4`), shared by both reviewers, each blind to the other's review; commit 60bf87b1d8. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (7 SHOULD-FIX, 2 NIT).
- **Disposition:** taken in the round's fix commit (named in round 2's records):
  - Set for this app closes the read-only branch a base's or package's cell opened, so its guard isn't Set's. The test now opens the package's cell first, then sets (1).
  - radius.full's pill starts Set from 9999px, a length the server takes, and says so; tested, the proposal accepted (2).
  - The new test is in the async lane (4).
  - The test checks the field disabled or enabled, Set's button gone and back, and the grid after Back before its undo (5).
  - Set's file is compared whole (6).
  - A proposal waits as long as a read (7).
  - The package name is read from describe's `ir.tokens` (8).
  - The fixture ships the web (the theme fixture's web crate), and the displayed column order is compared with describe's targets (9).
- **Not taken:** explain's `bases` in the branch editor (3). `names` already lists every token a value reaches, base names included; `bases` is their base-name subset, which the tool doesn't need yet.

No BLOCKERs found. Back, Back’s undo, and package Set/Back compare complete snapshots recorded after real server replies. The `full` source label is correct; dropping the invented `stretch` fixture is appropriate because that kind remains reserved.

1. **SHOULD-FIX — Selecting a package cell before Set leaves Apply disabled.** The new drive [sets the package token before opening its cell](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:55), avoiding an existing tool defect. Open `reading.measure`, then choose “Set for this app”: `setForApp` retains the selected read-only branch, so [Apply’s guard](/tmp/rv-dt-s11f/apps/design-tool/app.contract:1906) disables the proposed override. Exercise that natural sequence and distinguish the explicit override from editing the package’s branch.

2. **SHOULD-FIX — `radius.full` displays correctly but cannot be Set.** Existing [initialization of the override value](/tmp/rv-dt-s11f/apps/design-tool/app.ts:671) copies the column’s wire value verbatim. For `radius.full`, that submits `value: "full"`, which the server does not accept as an authored token value. The [new assertion](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:70) checks only “the pill.” Supply a valid writable value and cover its proposal and application.

3. **SHOULD-FIX — The branch editor still discards `explain.bases`.** The existing [branch reader](/tmp/rv-dt-s11f/apps/design-tool/app.ts:748) carries only `names` and `dials`. For `card.pad: length = space.md`, with `space.md` unset, the server reports the dependency in `bases`; the tool consequently treats the value as having no expression inputs and offers no base dependency link or pinning explanation. Consume `bases` with its documented all-branches semantics and add an alias case to the fixture.

4. **SHOULD-FIX — The replacement test is absent from the async lane.** [The script selection](/tmp/rv-dt-s11f/scripts/async.mjs:114) explicitly includes `design-tool.test.mjs`, but neither discovers nor imports `design-tool-scale.test.mjs`. Routine async runs therefore never execute this replacement. Add it to the existing glue selection.

5. **SHOULD-FIX — Read-only/editable and button-transition coverage was lost.** The [new assertions](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:90) check the package’s message and the override’s displayed value, not field disabled/enabled state. A package field accidentally becoming editable—or the app override remaining disabled—passes. Restore those assertions, the newly Set base cell’s editability, and Set-button disappearance/reappearance. In particular, there is no tree captured between base Back and its undo, so the restored base UI is never checked.

6. **SHOULD-FIX — Set’s “one line” assertion is weaker than its claim.** [Line 81](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:81) filters out every line already present in the original, ignoring their order, multiplicity and removal. An extra blank inserted alongside the token is invisible to this assertion; Back’s blank cleanup can remove it, leaving the later round-trip checks green. Compare Set’s entire expected file, as the package Set assertion already does.

7. **SHOULD-FIX — The existing proposal timeout remains a real-server timing risk.** [The proposal request](/tmp/rv-dt-s11f/apps/design-tool/app.ts:782) uses [the client’s default 15-second timeout](/tmp/rv-dt-s11f/apps/design-tool/wire.ts:109), despite other design reads allowing 130 seconds for the serialized server. A healthy proposal queued beyond 15 seconds becomes a “No design server” error and the drive fails at Apply. Align that wait with the server budget. The `clock +1` steps themselves correctly advance the one-millisecond follow task; they are not arbitrary sleeps.

8. **NIT — The package-name lookup reads the wrong field.** [Line 45](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:45) reads `described.tokens`; the real shape is `described.ir.tokens`. It therefore always uses the hard-coded fallback. Read the actual row and assert that its package metadata exists.

9. **NIT — The fixture loses shipped-web coverage and does not check column order.** [Copying only the manifest](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:25) produces iOS/Mac targets: the fixture’s web target comes from its uncopied `web/Cargo.toml`. Neutral is now tested only behind expansion, unlike the dropped test. Also, [individual ID lookups](/tmp/rv-dt-s11f/scripts/design-tool-scale.test.mjs:66) accept reversed column order. Declare the web target explicitly and compare the displayed column sequence against `describe.targets`.

Static review only; no files changed, builds or test suites run.
