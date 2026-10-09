---
name: 20261008-app-contract-root-rewrite
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-app-contract-root-rewrite
pr_url: https://github.com/ccheever/exact2/pull/332
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
  [app-contract-room](20261008-app-contract-room.md) (#264) moved the root's view into `T3Window`
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

- Parent: [spec](../../spec.md), [plan](../../plan.md).
- Prior work: [app-contract-room](20261008-app-contract-room.md): the 1,459-line analysis, plan identity by
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

- 2026-10-08: started at `96c4c38f2`; analysis and baseline; paused at the coordinator's wrap-up (`2cdd8a864`).
- 2026-10-08/09: resumed; one commit per area, the clone checks after each (below); #329 merged into the feature branch
  (`b53cd7da7`) and was merged here (`00043f245`) after the area commits; the live drive, the X67 probes, the plan
  comparison, the AppKit binaries and the five checks; draft PR.

**Result.** `app.contract` 1,488 → 1,230 lines at the merge (1,468 → 1,230 against `b53cd7da7`, which carries #329's
−20): −238, so 270 lines of room under the cap. The estimate was about 1,220. The rest of the root is what Charlie's #108
ruling keeps there: 46 resources, 16 mutations and 20 tasks (unchanged); the state they read; the state their `then` and
task actions write (`commandCompleted` resets `keybindingOpen`, `welcomeLink`, `paletteScroll`, … and, since #329,
calls `openSettings()`, which writes `settingsMenu` and `sidebarHoverId`); and the command dispatch. Three moves were
measured and left in the root (below), so the count stays above 1,200 by design.

| Area | Moved to | What moved | Root lines |
| --- | --- | --- | --- |
| Retired files | — | `panels.contract`, `settings-panels.contract` removed (and their `font-size-map.json` rows) | 0 |
| Welcome, banner, pull request widths | `WelcomeLayer`, `ChatColumn`, `PagesCover` | `welcomeHelpOpen` + `welcomeToggleHelp`; `dismissedProviderBanner` + `dismissProviderBanner`; derives `prPanelWidth`, `prListWidth` | −8 |
| Settings | `SettingsWindow` | `settingsEscapeHeld`, `settingsLegacyOpen`, `settingsRestoreOpen`, `keybindingSearch`, `licenseSearchOpen`; `coreLegacy`, `coreRestoreOpen/Close/Confirm`, `coreSlide`, `searchKeybindings`, `keybindingSearchKey`, `openLicenseSearch`, `closeLicenseSearch`, `licenseSearchKey`; window halves of `editSettingsQuery`, `coreSearchKey`, `coreNavigate`, `corePick` (`coreChange`'s last arm calls `settingsCommand`) | −51 |
| Sidebar | `T3Window` | `sidebarHoverLast/Y/H`, `sidebarScrollY`, 13 `drag*` states, 5 drag derives; `sidebarScrolled`, `sidebarDrag`, `sidebarDragEnd` (drops through `sidebarRun("drop", …)`); `sidebarHover`'s geometry | −63 |
| Title menu, panels | `T3Window` | `titleMenuOpen/FromTitle/Serial/X/Y`, `renaming`, `renameText`, `rightMaximized`, `detailsEditors`; `titleUi` (root `titleSend` for its send); window halves of `titleMenuPick`, `panelUi`, `detailsAct` | −41 |
| Add environment | `T3Window` | `credential`, `routeTarget`, `routeLabel`; `openRoute`; window halves of `openConnection`, `closeConnection`, `closeModal`, `editOrigin`, `editCredential` | −18 |
| SnapShot setup | `T3Window` | `snapshotSetupOpen`, `snapshotSetupWasEnabled`; `toggleSnapshots` (→ `settingsCommand`), `showSnapshotSetup` (root `clearModalError`) | −16 |
| Settings editors | `T3Window` | `restMenu`, `restLabel`; `restMenuOpen`; `rest` and `restInput` as the window's names for the root's `restRaw`; window halves of `restOpen`, `restClose`, `restRaw` | −21 |
| Add provider | `T3Window` | `wizardAttempted` (window halves of `providerUi`, `providerAdd`) | −8 |
| Composer | `T3Window` | derives `canRest`, `resting`, `composerLeft` | −3 |
| Sidebar sends | `T3Window` | `toggleSidebar`, `finishResize`, `search` through the root's `command` dispatch (`sidebar`/`search` → `localChanged`) | −6 |
| Root only | — | `liveTick` calls `dispatchTimelineReads()` | −3 |
| Docs | — | AGENT-HANDOFF "Where a feature adds its code", README "Source and checks", `app-window.contract` header | 0 |

Kept in the root after measuring:
- **The fold hold** (`foldHold`, `releaseFold`, `chatLocal`'s fold line): a window wrapper around `chatLocal` is inlined
  at every call inside the timeline's and the panels' repeated rows: +5.46 MB of plan for 7 lines.
- **The connections' remove confirm and row menu** (`connectionAskRemove`, `connectionRemove`, `connectionMenu`,
  `connectionToggleMenu`): as a window action, `connectionAskRemove` makes the build refuse
  `connections-network.contract:167:5 [type-cannot-infer] cannot infer the type of parameter @capture:…:a3:0` (the
  `ask` prop of `NetworkAccessRows`, reached through `SettingsWindow` › `ConnectionsPanel` › `ThisMachineSection`). Three
  minimal repros of that prop chain (an action prop of a child's action, passed down and captured with an argument,
  also in a child component's prop and an intermediate action) compile, so the trigger is not isolated; recorded as a
  finding with this exact refusal; no workaround in the connections files, no local issue number claimed.
- **`popoverSession`, `settingsThemeRequest`, `noticeExitTarget`, `providerCreated`, `draft`**: written through
  `coreChange` → `openModels()`, computing a root value, or read by a task, `then` or root send.

**Plan comparison** (`contract build --map`, slots and actions by declaring component; against `b53cd7da7`):

| | `b53cd7da7` | branch | difference |
| --- | --- | --- | --- |
| slots | 5,512 (T3Code 186, T3Window 6) | 5,512 (T3Code 145, T3Window 40, SettingsWindow 5, ChatColumn 3, WelcomeLayer 1) | 41 slots move from the root to their owners; every other component's slots are unchanged |
| actions | 6,715 (T3Code 176, T3Window 4) | 6,735 (T3Code 153, T3Window 31, SettingsWindow 14, ChatColumn 2, WelcomeLayer 1) | +20: the window halves that wrap a root half |
| derives (root) | 20 | 10 | a child's derive is inlined at each read |
| nodes / regions | 88,221 / 36,005 | 88,221 / 36,005 | none: no view moved |
| resources | 46 | 46 | none |
| plan bytes | 20,139,299 | 21,128,043 | +988,744 (+4.9%): the wrappers and derives are inlined where children call or read them (by step: sidebar +363 KB, title menu and panels +470 KB of which `titleMenuPick` 179 KB, settings editors +159 KB, Add provider +88 KB, sidebar sends +26 KB; the others ±30 KB) |

**X67 ([#320](https://github.com/ccheever/exact2/issues/320)), a note only.** The same probes as #320's record §5, the debug
compiler on the clone under `ulimit -s`: base `96c4c38f2`: 2048 overflows its stack (`thread 'main' has overflowed its
stack`), 3072 and 4096 compile; branch: 2048 exit 134 (stack overflow), 3072 and 4096 compile. The deepest site in the plan
(nodes and regions, the plan's own `validate_site_depth` walk) is 79 on both, and the deepest component chain is 20. The
rewrite moves no view, so it does not lower the nesting; X67 stays as #320 describes.

**Live drive** (agent mode, 1280×840; base `96c4c38f2` from the evidence-base worktree under its `.build-lock`, branch at
`cbdc478ba`, same script and lane; paired with the real-GitHub lane's primary server on 16620): Welcome (pairing help,
pairing with a single-use link, Agents, Projects, skip import), Settings › Connections, Add environment (opened, closed),
Settings › Providers, the settings search, a new thread with text in the composer, the Pull Requests list, #116's panel
on Summary, Code and Timeline. 11 of 16 screens are pixel-identical; the pull request screens differ only by live GitHub
data between the runs; the Agents step by a hover highlight under the resting agent pointer. Record: `drive-record.md` on
`t3-code-evidence/app-contract-root-rewrite/`.

**Not done / not verified.**
- Pinning a Usage segment's popover: blocked by a missing signed-in provider on the lane. The extra drive (below) reached
  Usage from the home page, but the page has no segment to pin ("Codex: Could not read limits." on Limits), in base and
  branch alike; it needs a Codex or Claude login with limits on the server the app reads.
- The `connectionAskRemove` compiler refusal above (not isolated; the move is not made).
- The README's `timeline-keyboard` recipe: broken on the base too (Swift access level and key API); tracked as a
  separate follow-up (coordinator, 2026-10-09). ExactKit now uses Swift's `package` access level; with `-package-name`
  added, `macos/tests/timeline-keyboard/main.swift:31` passes a `KeyPress` where ExactKit expects `String`. Neither
  `host/` nor that test changes here. Not run.

**Extra drive** (the coordinator's go-ahead, 2026-10-09; Usage and the thread title menu only; base `b53cd7da7`, branch
`a3b9b094f`, the same script and lane; one titled thread created on the lane server by RPC first): Usage from the home
page's sidebar, its Limits view; the thread's title menu, Rename thread (the field with the title), Escape (the title,
unchanged). All six screens pixel-identical; record `drive2-record.md` on `t3-code-evidence/app-contract-root-rewrite/`.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| baseline | `96c4c38f2` | `bun test examples/t3-code --timeout 60000`: 3462 pass, 1 skip, 0 fail (253 files; the 6924/2/506 first measured counted a scratch copy of the sources twice); `contract build`: 5512 slots, 20 derives, 46 resources, 6715 actions, 88221 nodes, 36004 regions, 20136518 bytes (debug and host-dev give the same plan); root slots 186 (170 states + 16 mutations), root actions 176; T3Window 6 slots, 4 actions | — | — |
| X67 probe, base | `96c4c38f2` | deepest plan site (nodes and regions, the plan's own `validate_site_depth` walk): 79; deepest component chain 20 (`TimelineIcon` … `RightPanels`, `T3Window`, `T3Code`); debug compiler, `ulimit -s 2048`: `thread 'main' has overflowed its stack` (as #320's record §5); 4096 and 3072 not run (stopped at the wrap-up) | — | — |
| area commits | `ca7ab7304` … `cbdc478ba` (11 commits) | after each: `bun test examples/t3-code` 0 fail, strict `tsc` clean, `contract build` OK, caps OK; slots 5,512 at every step; app.contract 1,488 → 1,480 → 1,429 → 1,366 → 1,325 → 1,307 → 1,291 → 1,270 → 1,262 → 1,259 → 1,253 → 1,250 | per-commit messages | — |
| measured, kept in the root | — | a `chatLocal` window wrapper: plan +5.46 MB for 7 lines (reverted in the same commit); `connectionAskRemove` as a window action: `connections-network.contract:167:5 [type-cannot-infer]` (three minimal repros compile) | this record, "Kept in the root" | the refusal is not isolated |
| merge #329 | `00043f245` | two conflicts (coreScope, providerAccent/providerAdd) resolved into the new structure; `providers-scope.test.ts` 6 pass; bun 3468 pass, 1 skip, 0 fail (254 files; no test lost or changed status against the base's JUnit list, plus #329's 6); tsc clean; contract build 5512 slots, 21,128,043 bytes; caps OK; app.contract 1,230 | merge commit | — |
| live drive, branch attempt 1 | `cbdc478ba` | stopped at its first step: the fresh home's Welcome wizard makes the sidebar inert (script); nothing paired | `drive-record.md` | — |
| live drive, branch retry and base | `cbdc478ba`, `96c4c38f2` | the same steps and results; 11/16 screens pixel-identical, the rest live GitHub data or a hover highlight | 11 before/after pairs, `drive-record.md` | Usage and the title menu not reached (script) |
| X67 probe, branch | `cbdc478ba` | 2048 exit 134 (stack overflow), 3072 and 4096 compile; deepest site 79; base 3072 and 4096 compile | this record | X67 (#320) |
| final checks | `c8ef3f6f7` | `cargo test -p t3-code-macos --lib` 13 pass (`title_snooze_tests` reads the moved `titleMenuOpen#1`; first run 12/1 before that); AppKit binaries 33/33 (`mermaid` with `T3_SERVER` on the lane server); five checks: build OK, `cargo test` 3,521 passed / 0 failed / 34 ignored in 94 binaries, clippy and fmt clean, caps OK, boot OK | PR body | `timeline-keyboard` recipe pre-existing break |
| extra drive (go-ahead 2026-10-09) | `b53cd7da7`, `a3b9b094f` | Usage reached from home; title menu, Rename, Escape: the same in both, 6/6 screens pixel-identical; no Usage segment to pin (no signed-in provider) | 5 before/after pairs, `drive2-record.md` | the pin: a signed-in provider |

## Next action

2026-10-09 (records sync, `t3-code-records-337`): merged into `feat(example)/t3-code` as #332 (`f633671b7`); the record moved to `tasks/closed/`. Its open rows stay as recorded above; the `timeline-keyboard` recipe was fixed by #333.

Review of the draft PR, then the coordinator's records sync. Open rows: the Usage pin (a signed-in provider on the lane),
the `connectionAskRemove` refusal (recorded), and the `timeline-keyboard` recipe (a separate follow-up).
