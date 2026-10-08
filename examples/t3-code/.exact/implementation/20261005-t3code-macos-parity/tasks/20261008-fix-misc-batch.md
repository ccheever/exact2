---
name: 20261008-fix-misc-batch
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-fix-misc-batch'
pr_url: https://github.com/ccheever/exact2/pull/306
verified_commit: 08b8ded8a
---

# Fixes from the real-input batch: a minimized window, the model picker's empty state, the update-branch storm, undo

## Outcome

Four clone bugs that the real-input batch ([#298](https://github.com/ccheever/exact2/pull/298)) found in merged
tasks are fixed to match T3 Code `1e2ecbd975`. A fifth (bug 9) moved to `fix-hover-cards`. The numbers are #298's:

- **7** ([app-activation](20261005-app-activation.md), #254): with the window minimized, `t3 app <dir>` timed
  out and the window stayed in the Dock. The reference restores and focuses it.
- **8** ([settings-model-picker](20261007-settings-model-picker.md), #246): a model search with no match
  showed no "No models found".
- **9** (#246): an unavailable model row had no tooltip. **Moved to fix-hover-cards** (coordinator decision,
  2026-10-08). That task owns every clone tooltip and hover card. Handoff below.
- **15** ([pr-header-actions-and-stacks](20261005-pr-header-actions-and-stacks.md), #262): after "Update with
  rebase", GitHub took the write, but no toast came. The panel kept "out-of-date by 8", and about 200
  invalidate/detail/list calls followed in 47 s.
- **3** ([editable-font-prompt-preview](20261007-editable-font-prompt-preview.md), #257, and the composer):
  ⌘Z, ⇧⌘Z and Edit › Undo did not undo typing.

## Scope and exclusions

Included: the cause and a reference-matching fix for 7, 8, 15 and 3, each with a regression test that fails on
the base. Each also has a live row: agent mode, or real input under the shared lock where the bug is about real
keys or windows. Each has before/after evidence. Bug 3's framework cause is recorded as X63.

Excluded:
- Bug 9 (moved).
- The other #298 bugs, which belong to other fix tasks.
- Framework edits: X63 is reported, not changed.
- The oracle and trace tools (user decision 2026-10-06).

## Causes and fixes

| Bug | Cause (reproduced) | Fix | Reference |
| --- | --- | --- | --- |
| 7 | `T3AppControl.activateMainWindow` chose its window among `NSApp.windows.filter { $0.canBecomeMain }`. AppKit answers `canBecomeMain` false for a window that is not visible, so a minimized window was never a candidate. Nothing was deminiaturized, and the page in the Dock never handled the handed request. Base: `request-timeout` after 16.09 s, still minimized, Finder in front | `activationTarget(main:windows:)` chooses by kind: titled and not a panel. It takes the main window, else a visible one, else a minimized one, and deminiaturizes it. A hidden app (⌘H) is unhidden first | `ElectronWindow.ts` `focusedMainOrFirst` → `if (window.isMinimized()) window.restore()`, `show`, `focus` |
| 8 | "No models found" sat inside the list's `scroll`. That scroll is `flex=0` with `min-height=0`, so it has no height when nothing matches. The text was in the tree but clipped away | The text is the list's sibling, at the scroll's depth (`model-picker.contract`) | `ModelPickerContent.tsx` `<ComboboxEmpty>No models found</ComboboxEmpty>` after the list's div |
| 15 | `readDetail` sent `pullRequests.invalidate` while `panel.invalidate` was set, and cleared the flag only after the detail read returned. The invalidate's server announcement asks the resource again, and Exact lets the running answer go (`let-go.ts`). The next run found the flag still set and invalidated again, which announced again. Base: 157 invalidates, 157 detail and 150 list reads. The toast showed at 40.6 s and the header still said "out-of-date with main by 31" | The flag is taken as the call goes out: one invalidate per host refresh, as the list's Refresh already does (`pages-prs.ts`, "One invalidate per Refresh press") | `PullRequestDetailPanel.tsx` `finishAction`: success toast, then `refreshFromHost` (one `invalidate`, then `refreshDetail`) |
| 3 | Exact's `textarea` keeps its own undo manager (host `TextAreaMac.swift`, `textUndo`). The clone's Edit › Undo (`R8KeysMenus.swift`) sent `undo:` to nil. No `NSTextView` answers `undo:`, so `NSWindow` did, on the window's manager, where none of the text's steps are. The item read enabled and did nothing. Edit › Redo was the host's own and failed the same way. X63 is the framework half: the host's own Edit menu has it for every plain textarea | Edit › Undo and Edit › Redo (⌘Z, ⇧⌘Z) act on the focused text view's own history. They leave it alone while text is composing (marked text). With editable text focused, ⌘Z is never thread.undo | Electron's `undo`/`redo` roles act on the focused editor. `keybindings.ts` binds `thread.undo` `when: "!terminalFocus && !editableFocus"` |

Files:
- `modules/apple/T3AppControl.swift`
- `model-picker.contract`
- `pages-pr-detail.ts`
- `modules/apple/R8KeysMenus.swift`

Tests:
- `pages-pr-actions.test.ts`
- `settings-model-picker.test.ts`
- `macos/tests/app-control/control.swift`
- `macos/tests/r8-keys/main.swift`

`app.contract` is untouched (1,478 lines after merging `ec32c8c37`).

## Bug 9 handoff (moved to fix-hover-cards)

- **Reference spec.** `apps/web/src/components/chat/ModelListRow.tsx`: a row with a `disabledReason` is wrapped in
  `<Tooltip><TooltipTrigger render={row} /><TooltipPopup side="left" align="center">{disabledReason}</TooltipPopup></Tooltip>`.
  The whole row is the trigger. `ModelPickerContent.tsx` wraps the picker in `<TooltipProvider delay={0}>`, so the
  tooltip opens at once. Base UI flips it to the right when the left has no room.
- **Exact text.** It is the clone's existing `row.reason` (`settings-core.ts` `scopedModelReason`), for example "This model is unavailable on
  Daehyeon’s MacBook Pro. Select that environment to choose its model separately." (`useScopedModelAvailability.ts`).
- **The row.** It is dimmed to 64% and its star is disabled (both already on the base). The base also carries
  `title=row.reason`, a native tooltip that never showed.
- **Agent row used.** Two lane servers ([mlane.mjs](https://raw.githubusercontent.com/ccheever/exact2/4cd436538f460b02b0940f3def7e517c21f25772/fix-misc-batch/mlane.mjs.txt)):
  - A on 16113, with Codex and Claude ready on placeholder keys.
  - B on 16114, with `claudeAgent` disabled.

  The app runs in agent mode at 1280×840. It pairs A through the welcome wizard and B through Settings ›
  Connections › Add environment (`server-origin` + `connect`). Then: Settings › General › Model
  (`setting-default-model-model`) at All projects / All environments → `provider-claudeAgent` → hover
  `model-claude-opus-5-5` (`s.tap(id, { hover: true })`). Script: [drive89.mjs](https://raw.githubusercontent.com/ccheever/exact2/08ad78765bc5b5d39517ed2eef45131270c672d7/fix-misc-batch/drive89.mjs.txt).
  The removed local prototype (frame() of the row, TipCard over the popover) rendered as in
  [03](https://raw.githubusercontent.com/ccheever/exact2/19ba21bce4c79a4748eab831ba8ace5a92480e09/fix-misc-batch/03-bug9-handoff-prototype.png).
- **Review notes on that prototype, for the hover layer.** A row removed under a resting pointer (search, rail,
  legacy toggle) hears no leave on macOS, so the tip must also check that its row is still listed and still
  unavailable. Moving onto the row's star sends the row a leave, but in the reference the whole row is the
  trigger. `HoverLayer` places top and bottom only; the reference here is `side="left"`.

## Acceptance and reproduction

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| 7: regression test fails on the base | pass | `AppControlActivationTargetTests` (2 tests). The base has no seam, so the AppKit-fact test (`canBecomeMain` false for a window not on screen) shows why its filter missed | — |
| 7: real CLI with the window minimized (real input, normal launch) | pass | branch: exit 0 in 1.21 s, `AXMinimized` false, lane app frontmost, beta's draft opened. Base: `request-timeout` after 16.09 s, still minimized ([record](https://raw.githubusercontent.com/ccheever/exact2/3b648c54cdaa8928eb2f4dbc9a73f96e46c8b876/fix-misc-batch/real-input-record.txt), [06](https://raw.githubusercontent.com/ccheever/exact2/ffea1c27f46fe05ee836df04439dccf6bcd4cf41/fix-misc-batch/06-minimized-window-restored-after.png) after-only) | — |
| 8: regression test fails on the base | pass | `settings-model-picker.test.ts` "\"No models found\" is the list's sibling…" (fails on the base contract) | — |
| 8: search with no match (agent, model lane) | pass | [02](https://raw.githubusercontent.com/ccheever/exact2/d33a3344159dcec4e7cc76da1a82c482be418c9a/fix-misc-batch/02-no-models-found-before-after.png): base empty, branch "No models found" ([record](https://raw.githubusercontent.com/ccheever/exact2/f951676d91abed5deb64588be3b9fa78600ff3a3/fix-misc-batch/model-picker-record.txt)) | — |
| 9: tooltip on an unavailable row | moved | handoff above | moved to fix-hover-cards (coordinator, 2026-10-08) |
| 15: regression test fails on the base | pass | `pages-pr-actions.test.ts` "Update with rebase: one invalidate however often its announcement lets the read go, then the fresh header": base 7 invalidates, branch 1 | — |
| 15: real GitHub (agent, lane server 16110, fresh #166/#167 31 behind) | pass | [01](https://raw.githubusercontent.com/ccheever/exact2/04fdd6c9b41a6a844e320b9efe04bcd2082af83a/fix-misc-batch/01-update-with-rebase-before-after.png). Base: toast at 40.6 s, still "out-of-date by 31" at +50 s, 157 invalidate / 157 detail / 150 list. Branch: toast at 7.5 s (the write itself takes ~6 s), mark gone, 1 / 2 / 2. GitHub `behind_by` 0 in both ([record](https://raw.githubusercontent.com/ccheever/exact2/5d20a6f173aa6ab76672346e90ecd449503c7b20/fix-misc-batch/update-with-rebase-record.txt), [drive15.mjs](https://raw.githubusercontent.com/ccheever/exact2/3fa241425041d4749ae44bdf6eeabdb198ec7bbe/fix-misc-batch/drive15.mjs.txt)) | — |
| 3: regression test fails on the base | pass | `R8KeysTests.testUndoAndRedoActOnTheFocusedTextsOwnHistory`. Its first assertions show the old route (`undo:` through the responder chain leaves the text); then ⌘Z, Edit › Redo, Edit › Undo, Edit › Redo, a composition left alone, and no thread.undo while text is focused | — |
| 3: composer, real keys (real input, normal launch) | pass | [04](https://raw.githubusercontent.com/ccheever/exact2/1209a2b1ee64709c1834c6baabfc304977453b61/fix-misc-batch/04-composer-undo-before-after.png). Base keeps "123" after real ⌘Z and Edit › Undo. Branch undoes it, Edit › Undo brings back an earlier deletion, ⇧⌘Z redoes. `R8_KEYS_LOG`: responder `TextArea`, "sent Undo target=R8KeysMenus" | — |
| 3: prompt preview, real keys | pass | [05](https://raw.githubusercontent.com/ccheever/exact2/5e86dfa9007112a975eddc878de195a94adafa9b/fix-misc-batch/05-prompt-preview-undo-after.png) after-only: ⌘Z removes the typed "12" (caret kept), ⇧⌘Z restores. Before: #298's real-input batch ([efp-undo12](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/editable-font-prompt-preview/02-efp-undo12.png)) | — |
| X63 (framework half of 3) | reported | [x63-repro](https://raw.githubusercontent.com/ccheever/exact2/56357628645f3ab2f7cd92d37217229808492ae9/fix-misc-batch/x63-repro.txt): the host's own `TextArea` on main `fa965d3e2`. `undo:` through the responder chain is handled by `NSWindow` and the text stays. [Issue draft](../issues/20261008-x63-textarea-undo-menu.md), compared with #275, #276 and #125 | — (not blocking) |
| Visual and protocol parity with the oracle | not run | — | user decision 2026-10-06: the desktop oracle and trace tools are not built |

## Progress

2026-10-08:
- Implemented on `feat(example)/t3-code-fix-misc-batch` from `23b19e2ec` with `c0475fbaa` and `ec32c8c37` merged.
- An independent review found nothing blocking. Taken from it:
  - the composition guard and `!editableFocus` for ⌘Z;
  - the hidden-app unhide;
  - for the bug 9 prototype (since removed): a tip cleared on every leave and when its row is gone.
- The coordinator moved bug 9 to fix-hover-cards; the prototype was removed and the handoff written above.
- Live rows:
  - Agent mode: 8 and 15.
  - Real input under the shared lock (07:16–07:24Z): 7 and 3, base and branch.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| repro (agent) | `c0475fbaa`, this worktree before any edit | #151 (10 behind): 237 invalidates, 237 details, 233 lists. Toast at 40.6 s, mark still shown after 50 s, though the server's own detail said up-to-date | update-with-rebase-record "First reproduction" | — |
| after 1 (agent) | fix on `c0475fbaa` | #150: 1 invalidate, toast after the write, mark gone | — | — |
| pairs (agent) | base `c0475fbaa` (evidence-base, under its lock) vs branch | 15 on #166/#167; 8 and the bug 9 prototype on the model lane | 01–03 | — |
| X63 repro | host on main `fa965d3e2` | the host's `TextArea`: `undo:` handled by `NSWindow`, text unchanged; ⌘Z key equivalent not taken (no markup) | x63-repro | — |
| review | `829a0d39f` | no blocking findings; the changes listed under Progress | — | — |
| real input (the one session) | branch `08b8ded8a` and base `c0475fbaa`, lane copies, normal launch | 7: pass, base reproduced. 3: composer and preview pass, base reproduced. One click may have landed on fix-provider-auth-state's agent window, which overlapped mid-session | 04–06, real-input-record | — |
| checks | `9ae8c825b` + `08b8ded8a` | `bun test examples/t3-code` 3135 pass / 1 skip / 0 fail (two new tests). Strict tsc clean. Contract build 3931 slots, 46 resources. AppKit: app-control 27/0, r8-keys 5/0, menus 45/0, composer 52/0. `cargo test -p t3-code-macos --lib` 13 passed. Caps within. Five checks on `0bc4115a2`: build 0; test 3,521 passed / 0 failed / 34 ignored (94 binaries); clippy 0; fmt 0; boot 0 | PR body | — |

## Next action

The coordinator reviews and merges draft PR #306. fix-hover-cards picks up bug 9 from the handoff. X63 waits for the
user's approval to publish.
