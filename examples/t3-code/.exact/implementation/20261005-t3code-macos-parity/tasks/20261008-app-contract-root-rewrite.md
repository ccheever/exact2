---
name: 20261008-app-contract-root-rewrite
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-app-contract-root-rewrite
pr_url: null
verified_commit: null
---

# Rewrite app.contract so the root stays well under the 1,500-line cap

## Outcome

The root component `T3Code` in `app.contract` holds only what the compiler keeps in the root: resources, mutations,
tasks, the states they read, and the actions that write that state or send a root mutation. Every view-only state, and
the view-only half of each action, lives in an area child component in the area file that already draws that area.
`app.contract` ends at about 1,200 lines or less (300 lines of room or more). What a person sees and does is unchanged.

User decision (2026-10-08): `app.contract` is to be rewritten so the root stays well under the cap. The task starts only
after the PRs in flight merge: #290 (popover-escape-parity), #307 (fix-hover-cards), #310 (fix-keyboard-focus), #312 (fix-provider-auth-state), #308 (pr-code-tab) and #311 (pr-links-previews-and-routing). (`fix-misc-batch` merged as #306.)

Charlie's ruling on [#108](https://github.com/ccheever/exact2/issues/108#issuecomment-6055587663) (2026-10-08) endorses this rewrite
as the remedy: "Use child state/actions to reduce the root now; defer request ownership to the existing D5 design. … Use the
T3 Code root rewrite already planned in #303 as the immediate remedy. … do not add partial-root syntax merely to evade the cap."
Main PR [#327](https://github.com/ccheever/exact2/pull/327)'s audit (open on main, 2026-10-08) repeats it: "Immediate child-state/action
root rewrite belongs to #303/T3 example"; general child resources wait for D5.

## Why (measured at `ec32c8c37`, 2026-10-08)

- `app.contract` has 1,478 of the 1,500 lines `bun scripts/caps.mjs` allows, so 22 lines of room are left.
  [app-contract-room](closed/20261008-app-contract-room.md) (#264) moved the root's view into `T3Window`
  (`app-window.contract`): 1,499 → 1,459 lines. The merges since then added 19 lines. Every new feature still adds
  root state or actions, so the room runs out again within a few tasks.
- The root has 168 states, 46 resources, 16 mutations, 18 tasks, 20 derives and 175 actions. Its `view` is one
  `T3Window(...)` line with 312 props (8,355 characters).
- #264 found why a plain move stops there ("Why app.contract stops at 1,459"):
  - `resource`, `mutation` and `task` are root-only (X9, [#108](https://github.com/ccheever/exact2/issues/108)).
  - A child never assigns root state. It may call its `action` props, and the parent's action runs in the same commit
    (`docs/contract-for-agents.md`, "Composition and lifetime").
  - A root `derive` moved into a child is inlined at each read.
- What is left is the rewrite #264 named: split the actions that write both kinds of state.
- With #307 the file is at about 1,488 lines (its hover tasks add 10 root lines); the other in-flight PRs change it by
  0 net lines but edit root lines, which this task then moves.
- A rough count by script (names read by a resource, task or mutation header, directly or through a derive) gives:
  - 93 states that stay in the root;
  - 75 view-only states (#264 counted 74).
- Of the 75, 15 are written only by actions that send nothing and write no data-read state. They can move as they are:
  `welcomeHelpOpen`, `dismissedProviderBanner`, `connectionMenu`, `workspaceRetryKey`, `workspaceRetryAt`,
  `sidebarScrollY` and nine `drag*` geometry states.
- The other 60 or so need their writing actions split (#264: about 37 root actions):
  - a child half that sets the child's own state;
  - a call to a root action prop for the root half.

## Scope and exclusions

Included:

1. **Area owners.** For each area below, a child component (or the area's existing view component) owns that area's
   view-only states, their derives, and the view-only half of each action. The root passes it resources, root values
   and root action props. `T3Window`'s prop list shrinks to match.
2. **Action splits.** Each root action that writes view-only state and also writes data-read state or sends a mutation
   splits in two:
   - the child's action sets the child's state;
   - then, as its last statement, it calls the root action, which keeps the data-read writes and the `send`.
3. **What stays in the root.** Resources, mutations, tasks, the states they read, and the command dispatch that sends
   `changed`, `localChanged` or `terminalChanged` stay where they are.
4. **Area map.** Lines in `app.contract` by name prefix, approximate:

   | Area (name prefix) | Lines | Members | Destination (the area's view file) |
   | --- | --- | --- | --- |
   | Palette (`palette*`) | 136 | 10 states, 1 resource, 1 mutation, 2 derives, 8 actions | `palette.contract` (`PaletteView`) |
   | Settings (`core*`, `settings*`, `keybinding*`, `license*`, `wizard*`, `rest*`) | ~200 | 43 states, 3 resources, 1 task, 26 actions (16 of them `core*`) | `settings-core.contract`, `settings-keybindings.contract`, `settings-licenses.contract`, `providers-wizard.contract`, `settings-rest-dialogs.contract`, `app-settings.contract` |
   | Providers (`provider*`) | 81 | 6 states, 2 resources, 2 mutations, 1 task, 5 actions | `providers.contract`, `providers-setup.contract` |
   | Sidebar (`sidebar*`, `drag*`, `sb*`) | ~110 | 23 states, 6 derives, 3 resources, 1 task, 8 actions | `sidebar.contract`, `sidebar-row.contract`, `legacy-sidebar.contract` |
   | Thread title (`title*`) | 65 | 6 states, 1 task, 2 actions | `chat.contract` (the title and its menu) |
   | Pull requests (`pr*`) | 45 | 7 states, 2 derives, 3 resources, 1 mutation, 1 task, 10 actions | `pages-prs.contract`, `pages-pr-detail.contract` |
   | Welcome (`welcome*`) | 33 | 4 states, 1 resource, 8 actions | `pages-welcome.contract`, `app-overlays.contract` (`WelcomeLayer`) |
   | Toasts and notices (`toast*`, `notice*`) | 43 | 6 states, 1 task, 4 actions | `shell-toast.contract` |
   | Confirm and modal (`confirm*`, `modal*`) | ~25 | 6 states, 1 action | `app-overlays.contract` (`WindowOverlays`) |
   | Usage (`usage*`), SSH (`ssh*`), terminal (`terminal*`) | 16 / 20 / 21 | mostly resources, tasks and mutations | mostly stay; view-only states go to `pages-usage.contract`, `ssh-prompt.contract`, `terminal.contract` |
   | Command dispatch (`command*`, `open*`, `close*`, `edit*`, `select*`, `toggle*`, `focus*`, `menu*`) | ~280 | actions that send root mutations | stay; their view-only branches move to the area that opens them |

5. **Records.** `AGENT-HANDOFF.md` "Where a feature adds its code": new state goes to the area owner, not the root. The
   README's source paragraph. Any recipe, test or record that names a moved root state.
6. **Main's `now()` rename.** Main `9731c8056` (LLP 1109 D1) renames `now()` to `performanceNow()` and refuses `now()`
   (`type-now-renamed`); the clone has 26 call sites (most in `app.contract`, 3 in `theme-color-picker.contract`, 1 in
   `r6-device.contract`). Only if main adoption round 7 has merged that main by the time this task starts, its first
   commit renames them; otherwise round 7 does the rename.
7. **Retired files.** Remove the comment-only `panels.contract` and `settings-panels.contract`: main's new examples test
   (`contract/cli/tests/it/button_migration.rs`, `cbd76f7d7`) compiles every `.contract` under `examples/` and refuses
   them (`analyze-no-component`).

Excluded:
- Framework changes. Charlie ruled on #108 (2026-10-08): child resources (X9's option A1) are not chosen, request
  ownership waits for LLP 1035.005.000 D5, and no partial-root syntax is added.
- Behavior, visual or protocol changes, and new features.
- `client.ts` and the other TypeScript files, unless a moved name is read there.

## Context and guidance

- Parent: [spec](../spec.md), [plan](../plan.md).
- Prior work: [app-contract-room](closed/20261008-app-contract-room.md): the 1,459-line analysis, plan identity by
  renumbering, the `T3Window` move.
- Contract rules: `docs/contract-for-agents.md` "Composition and lifetime" (children hold state, derives and actions,
  call `action` props, never write parent state, declare no resources, mutations or tasks); LLP 1091.
- Tests that read `app.contract`'s source: `auto-balance.test.ts`, `context-menu-hookup.test.ts`,
  `dialog-focus.test.ts`. They follow moved code to its new file and must not get weaker.
- The plan cannot stay byte-identical: state slots move from the root into child instances, and instance numbers
  shift (#264). The proof of no behavior change is the tests and a live drive, not a plan hash.

## Dependencies

| Kind | Item | State | Effect |
| --- | --- | --- | --- |
| merged task PRs | #290 (popover-escape-parity), #307 (fix-hover-cards), #310 (fix-keyboard-focus), #312 (fix-provider-auth-state), #308 (pr-code-tab) and #311 (pr-links-previews-and-routing) | in flight on 2026-10-08; merged: #290 (`84a52dde0`), #312 (`421047c46`), #308 (`0e2901aec`), #307 (`3c8c11ef2`), #310 (`f45eab04a`); #311 open | start after they all merge (user decision) |
| recorded decision | rewrite `app.contract` so the root stays well under the cap (user, 2026-10-08) | decided | this task |
| recorded decision | X9 ([#108](https://github.com/ccheever/exact2/issues/108)), Charlie, 2026-10-08 | decided | "Use the T3 Code root rewrite already planned in #303 as the immediate remedy": this task; no framework wait (resources, mutations and tasks stay in the root) |
| main adoption | round 7 (main `9731c8056`, the `now()` rename) | waits for main fix of X67 | rename in this task's first commit only if round 7 merged main first |

## Acceptance and reproduction

| Criterion | Check |
| --- | --- |
| Room | `wc -l app.contract` about 1,200 or less; no file over 1,500 (`git add -A && bun scripts/caps.mjs`) |
| Root keeps only root work | the root's 46 resources, 16 mutations and 18 tasks (or the counts after the in-flight PRs) are unchanged; every root state is read by a resource, task, mutation or `T3Window` prop it cannot do without; the view-only states listed in the PR moved |
| No behavior change | `bun test examples/t3-code` with the same test names passing as on the base (the three source-reading tests updated only to follow moved names); strict `tsc`; `contract build`; `cargo test -p t3-code-macos --lib`; the AppKit binaries the README recipe builds |
| Live | one macOS agent-mode drive over each moved area, base against branch, same fixture: palette open, search and run; Settings navigation, a keybinding edit, Licenses; the Add provider wizard; a sidebar drag between shelves; the thread title menu (rename, Custom snooze); Pull Requests list and detail; Welcome; Usage; a toast; a confirm dialog; the terminal close confirm; the SSH prompt. Rows that need a real pointer go to the next real-input batch |
| Repository | the five checks from `CLAUDE.md` with their flags |

## Progress

- 2026-10-08: started in worktree `t3-code-app-contract-root-rewrite` at `96c4c38f2` (#311 merged; #329 still open).
  Analysis and baseline done; no source edited yet. Paused on the coordinator's wrap-up (usage limit). No PR yet.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| baseline | `96c4c38f2` | `bun test examples/t3-code --timeout 60000`: 6924 pass, 2 skip, 0 fail (6926 tests, 506 files); `contract build`: 5512 slots, 20 derives, 46 resources, 6715 actions, 88221 nodes, 36004 regions, 20136518 bytes (debug and host-dev give the same plan); root slots 186 (170 states + 16 mutations), root actions 176; T3Window 6 slots, 4 actions | — | — |
| X67 probe, base | `96c4c38f2` | deepest plan site (nodes and regions, the plan's own `validate_site_depth` walk): 79; deepest component chain 20 (`TimelineIcon` … `RightPanels`, `T3Window`, `T3Code`); debug compiler, `ulimit -s 2048`: `thread 'main' has overflowed its stack` (as #320's record §5); 4096 and 3072 not run (stopped at the wrap-up) | — | — |

## Resume from here (2026-10-08, paused before any edit)

**First:** `git fetch origin`; if #329 (fix-providers-environment-scope) has merged, `git merge 'origin/feat(example)/t3-code'`
before editing (a scratch merge of #329's app.contract applies cleanly: 1,488 → 1,467 lines), and run `providers-scope.test.ts`.
#329 makes `commandCompleted` call `openSettings()`, so `settingsMenu` and `sidebarHoverId` become then-written root
state: they stay in the root (the plan below already assumes this).

**Measured classification** (script over the root; with #329): of 170 states, 96 are read by a resource, task or
mutation header, and 25 more are written by a task or `then` action (`confirm*`, `draftOwner`, `jumpRequest`,
`keybindingOpen`, `keybindingRecording`, `modalError`, `noticeExit`, `originEdited`, `paletteBusy`, `paletteScroll`,
`pendingModal`, `settingsMenu`, `settingsScrollTop`, `sidebarHoverId`, `sshResponding`, `sshReturnToConnect`,
`welcomeLink`, `welcomePairOpen`, `workspaceRetry*`). These stay. Also staying: `noticeExitTarget` and `providerCreated`
(read by a task or `then` action), `draft` (read by the root `send` through `composerText`), `popoverSession`
(written through `coreChange` → `openModels()`), `settingsThemeRequest` (computes a root state). About 44 view-only
states move.

**Owners.** No new component layer (keeps X67's depth). A state moves to an area component only when that component is
single, always mounted (no `when` above it in `T3Window`) and every writer is called only inside it; otherwise to
`T3Window` (Sidebar and SidebarOverlays have two or three instances under `when`, so their state goes to `T3Window`).
Naming: a wrapper keeps the public action name (so view lines and area components do not change) and the root half is
passed as `<name>Root` (for example `titleMenuPickRoot=titleMenuPick`); root actions keep their names (tests read them).

| Area | Moves | To | Root half |
| --- | --- | --- | --- |
| Sidebar | `sidebarHoverLast/Y/H`, `sidebarScrollY`, 13 `drag*` states, derives `dragCX/CY/Target/Verb/Offset`; `sidebarScrolled`, `sidebarDrag` (its `sidebarHoverId = ""` becomes `sidebarHoverRoot(sidebarHoverId, false)`), `sidebarDragEnd` (both sends are exactly `sidebarRun("drop", id, …)`) | `T3Window` | `sidebarHover`: `sidebarHoverId` and the search-hover send only |
| Title menu | `titleMenuOpen/FromTitle/Serial/X/Y`, `renaming`, `renameText`; `titleUi` view part; `titleMenuPick`'s first line and `ui:rename` | `T3Window` | new `titleSend(what, text)`: the commit/new-thread send block; `titleMenuPick` keeps settings, confirm and sends |
| Panels | `rightMaximized`, `detailsEditors`; `titleMenuOpen = false` in `panelUi`, `detailsAct` | `T3Window` | `panelUi`, `detailsAct` minus those lines |
| Fold | `foldHold`, `releaseFold`, `chatLocal`'s fold line, `scrollToEnd`'s `foldHold = false` | `T3Window` | `chatLocal` keeps its send (context-menu-hookup test) |
| Connections | `credential`, `routeTarget`, `routeLabel`, `connectionRemove`, `connectionMenu`; `connectionToggleMenu`, `connectionAskRemove`; `openRoute` = view writes + `openConnectionRoot()` | `T3Window` | `openConnection`, `closeConnection`, `closeModal`, `connectionOp` (its `connectionRemove = ""` sits under `if not commandPending`), `editOrigin`, `editCredential` minus view writes |
| Snapshot | `snapshotSetupOpen`, `snapshotSetupWasEnabled`; `toggleSnapshots` (first arm = `settingsCommand("setting-snapshot", "snapShotEnabled", "false")`), `showSnapshotSetup` | `T3Window` | new `clearModalError` |
| Rest | `restMenu`, `restLabel`; `restMenuOpen`; `rest` = `restRaw(`rest:${op}`, `${settingsEnvironmentId}:${settingsProjectId}`, value)`; `restInput` = `restRawRoot(…)` (it never cleared `restMenu`) | `T3Window` (new props `settingsEnvironmentId`, `settingsProjectId`) | `restOpen`, `restClose`, `restRaw` minus `restMenu`/`restLabel` |
| Settings | `settingsEscapeHeld`, `settingsLegacyOpen`, `settingsRestoreOpen`, `keybindingSearch`, `licenseSearchOpen`; `coreLegacy`, `coreRestoreOpen/Close`, `coreRestoreConfirm` (= `settingsCommand("settings-core", `restore-device-defaults:|${settingsCore.scopeKey}`, "")`), `coreSlide` (= `settingsCommand`), `searchKeybindings`, `keybindingSearchKey` (→ `editKeybindingQuery("")`), `openLicenseSearch`, `closeLicenseSearch`, `licenseSearchKey` (→ `editLicenseQuery("")`); view lines of `editSettingsQuery`, `coreSearchKey`, `coreNavigate`, `corePick` | `SettingsWindow` (new prop `settingsCommand`) | the rest; `coreChange`'s last arm can call `settingsCommand` |
| Providers | `wizardAttempted` (`providerUi` open/driver/manual/step lines, `providerAdd`'s first line) | `T3Window` | `providerUi`, `providerAdd` |
| Welcome | `welcomeHelpOpen`, `welcomeToggleHelp` | `WelcomeLayer` | — |
| Banner | `dismissedProviderBanner`, `dismissProviderBanner` | `ChatColumn` | — |
| Pull requests | derives `prPanelWidth`, `prListWidth` | `PagesCover` | — |
| Composer | derives `canRest`, `resting`, `composerLeft` | `T3Window` (new prop `composerFocused`) | — |
| Sidebar sends | `search`, `toggleSidebar`, `finishResize` (the root `command` dispatcher already routes `search` and `sidebar` to `localChanged`) | `T3Window` | — |

Estimate: about −245 lines (1,488 → about 1,240; with #329 about 1,220). Optional root-internal de-duplication if more
room is wanted: `liveTick`'s timeline block = `dispatchTimelineReads()` (−3). Inside the root only `providerAdd`,
`sidebarRun` and `command` are called by other root actions (checked), so the wrappers above catch every caller.

**Checks to keep in view:** the source-reading tests (`dialog-focus`, `hover-layer` (the `T3Window(data=data,
viewport=viewport, hoverWait=hoverWait, hoverHold=hoverHold,` prefix), `menu-keys`, `context-menu-hookup`,
`codex-setup`, `auto-balance`, `usage-pooled`) follow moved names only. Plan comparison: per-component slot counts
from `contract build -o p.plan --map` (`map.json` `slots[*].component`); expect T3Code −N and the owners +N, the plan's
root derives 20 → about 10 (child derives are inlined). X67: rerun the site-depth walk and the `ulimit -s` probe on the
branch (expect no change: no view moves). The scratch tools (`analyze.py`, `classify.py`, `uses.py`, `slots.py`,
`depthprobe/`, `x67/probe.sh`) are under this worktree's `target/arr/`, not committed. Then the per-area commits,
the clone checks after each, the live drive, and the draft PR as in the task prompt.

## Next action

Once #290, #307, #310, #312, #308 and #311 have merged into `feat(example)/t3-code`, run `prepare`:
- measure again, since those merges add root lines;
- confirm the area map above;
- list, per area, the actions that split, with their child half and root half;
- check whether round 7 has merged main (then the first commit renames `now()`);
- note only (an investigation, not a goal): check whether moving views into child components brings the clone's 103-site
  nesting below the debug build's 2 MiB limit of X67 ([#320](https://github.com/ccheever/exact2/issues/320), still open:
  #327 withdrew its attempt, so round 7 stays blocked). Record the result in the PR. #320's body says long component
  chains also overflow in optimized builds, so a lower count does not settle X67;
- then implement area by area, running the clone checks after each.
