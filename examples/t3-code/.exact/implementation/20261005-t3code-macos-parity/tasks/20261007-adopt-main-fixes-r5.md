---
name: 20261007-adopt-main-fixes-r5
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-r5
pr_url: https://github.com/ccheever/exact2/pull/236
verified_commit: null
---

# The clone adopts main's fixes, round 5 (#219, #220, #221, #223, #226; #113 checked)

## Outcome

The task branch records main `463acda68` in its ancestry and merges main `261dd4e10`, both as merge commits.
The PR must be merged with a merge commit, not squashed. Where a fix main merged covers what the clone
worked around, the workaround is gone and the clone behaves as T3 Code does. Where it does not, the issue files
name the open issue. The merge itself broke the clone: hooks were renamed access hatches, and LLP 1081 respells
names Exact invents. The clone now follows both. Main's linked-SDK check cannot build an app named
"T3 Code (Exact)" ([#234](https://github.com/ccheever/exact2/issues/234), filed here). `EXACT2-GAPS.md`,
`STATUS.md`, the issues README, the X11, X25, X26, X27 and X28 records and the task records with rows blocked by
them are current.

## Scope and exclusions

| Issue | Plan id | main PR | Clone sites |
| --- | --- | --- | --- |
| [#114](https://github.com/ccheever/exact2/issues/114) (rest: [#224](https://github.com/ccheever/exact2/issues/224)) | X28 | #219 | `shell-notify.ts`, `shell.ts`, `app.contract` `page`, `T3Notifications.swift`, `T3ActivityReporter.swift`, `T3Module+Shell.swift`, `shell-vcs.ts`, `shell-details.ts`, `snapshotSettings` |
| [#140](https://github.com/ccheever/exact2/issues/140) | X25 | #220 | `T3Sidebar`, `T3ComposerIntent`, `T3Menus`, `R8KeysLauncher`, `T3KeyRecorder`, `RightPanelTabsInput` (checked) |
| [#129](https://github.com/ccheever/exact2/issues/129) (rest: [#225](https://github.com/ccheever/exact2/issues/225)) | X11 | #221 | `ComposerDrawerLayer` and dialog backdrops (no edit), flattened glass (kept) |
| [#141](https://github.com/ccheever/exact2/issues/141) | X26 | #223, #226 | `sidebar-row.contract` ThreadMenu/DraftMenu, `sidebar-menu.ts`, `sidebar-commands.ts`, `r11-upstream-drafts.ts`, `sidebar-view.ts`, `T3Menus.swift`, `R8KeysMenus.swift` |
| [#113](https://github.com/ccheever/exact2/issues/113) | X27 | — (re-check) | `T3WindowChrome.swift`, `T3FullScreen.swift` (kept) |

Excluded: framework changes (two issues were filed instead), and notification actions and badges (#224, policy).
Also excluded: converting the Files tree's and the legacy sidebar project's right-click menus to context popovers
(see Next action).

## Context and guidance

Brief: coordinator's round-5 adoption task. The fixes were confirmed on main by their merge commits: `6af680b0e`
#219, `5a20af1cf` #220, `846a844da` #221, `d988e318b` #223 and `dfcf8e9cf` #226. PR bodies and issue comments
were read too: #140, #141 and #225 are open, #224 is open, #113 is closed with no comment. Main's vocabulary was
checked for X27: `env()` names, `host.macos.window`, `exactViewport()` and `exactPage()` fields. Earlier rounds:
[adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md) (#218),
[adopt-main-fixes-r3](closed/20261007-adopt-main-fixes-r3.md) (#207).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| ancestry | main `463acda68` (round 4 was squash-merged) | `463acda68` | recorded with `git merge -s ours`, no file changed | `f6faefc2d` |
| merged main | origin/main into the task branch | `261dd4e10` | merge commit | `830eadecd`: `Cargo.lock` kept the `t3-code-macos` entry (Cargo then dropped `sys-locale`, which nothing uses), `QUEUE.md` kept both sides; no file under `examples/t3-code` changed |
| records | `feat(example)/t3-code` | `fbce02624` (#231 records sync) | merged before the record edits | `efd2ed4b6`, no conflict |

## Decisions per issue

| Issue | Result | Why |
| --- | --- | --- |
| merge breakage (61b33c2ca, 0bd99f606) | **fixed in the clone** | `hook=` → `hatch=` (76 attributes), app.json `hatches`, `ExactHatches`/`ExactHatchKey`/`ExactElement.hatch` in the module and its AppKit tests, `timeline.test.ts`; LLP 1081's script turned `spring(` into `-exact-spring(` (shell-morph, two rotations) and `tint-color` into `-exact-tint-color` |
| #234 (new) | **filed**; local `otool` shim | `assertLinkedSdk` runs `otool -l "<app>/T3 Code (Exact)"`; otool reads `(Exact)` as an archive member and finds no SDK, so the build fails. `vtool -show-build` and `llvm-objdump` read the same file. The bundles here were built with a scratchpad `otool` that passes otool a symlink (not committed); renaming the app is not the clone's to do |
| #114 / X28 | **adopted** (issue stays open for #224) | Thread notifications choose toast or system notification by `page.hasFocus`. The activity reporter's `visible`/`focused` are the page's (`activityFacts`), held until known and reported on each change. The details card asks `vcs.refreshStatus` when focus returns (GitActionsControl's `focus` listener, which the clone had left out). SnapShot settings re-read on focus. `T3Notifications.active` and the reporter's AppKit window reads are gone; badge, click and sounds stay for #224 |
| #140 / X25 | **nothing to remove** (`closed-upstream`, #140 open) | Every monitor needs a window capture-phase handler or held-modifier state, or lives in a native view (terminal, device stream, composer) or the reference's own main-process code (held ⌘W, ⌘Q hold). Found: `press` already carries `MouseEvent` modifiers, which `T3ComposerIntent.take` and `T3Sidebar.pressModifiers` could use (not this round) |
| #141 / X26 (#223) | **adopted** for the sidebar | The thread row's and draft row's right-click menus are context popovers with nested submenu popovers. Each row carries its menu (`menuRows`; the bulk menu on a selected row), and a choice runs `menu-choice`/`draft-choice`. A real right-click shows the same `NSMenu`, and the agent can now open and choose the menu. Menus opened from the keyboard stay `T3Sidebar.swift`'s ([#235](https://github.com/ccheever/exact2/issues/235), filed here) |
| #141 / X26 (#226) | **adopted** | The clone's own Speech is removed (it would have shown twice). #226 also files the clone's ⇧⌘G and ⌘D buttons under Edit (Branch, Toggle Diff). `R8KeysMenus` hides Edit's app commands as File's, so Edit is DesktopApplicationMenu.ts's |
| #129 / X11 | **nothing to remove** (`closed-upstream`, #225 open) | The drawer's and dialogs' real blurs take #221 with no clone change. The flattened glass stays: nested backdrops and `saturate()` are #225 |
| #113 / X27 | **re-checked: still missing** | No `titlebar-area-*`, title-bar setting or full-screen fact on `261dd4e10`. No open issue tracks the rest (user decision) |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Clone builds on the merged base | — | `bun test examples/t3-code`, strict tsc (ES2023), `contract build`, `cargo test -p t3-code-macos --lib`, AppKit binaries, caps | green | macOS | numbers below |
| Focus is the page's | lane (below), agent | `prefer has-focus true/false/true`; `state`; trace proxy | `page.hasFocus` and `shell.notifications` follow; a report with `focused` follows each change; a git refresh when focus returns | macOS 1280×840 | record 05 |
| Row menu with a submenu | lane, two threads | `tap row-<id> contextmenu`, `tap thread-menu-copy`, `tap thread-menu-snooze`, `tap thread-menu-snooze:hour` | the menu and its submenu beside the row; the thread snoozes | macOS 1280×840 | images 01, 03 |
| Native menu | lane bundle copies (own bundle ids), real right-click | `orca computer click … --mouse-button right`, then Copy | the same `NSMenu` before and after, Copy ▸ open | macOS | image 02 |
| Edit menu | agent `state.menus` | — | Undo, Redo, —, Cut, Copy, Paste, Paste as Text, Delete, —, Select All, —, Speech (+ AppKit's) | macOS | record 05 |
| Backdrop edge | lane thread | `type composer "/"` | the drawer's edge as Chrome's | macOS | image 04 |

## Progress

Implemented 2026-10-07. Verification: unverified (task PR review pending).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | `830eadecd` (merged base, no clone change) | `bun test examples/t3-code` 2312 pass / 1 skip / 0 fail; strict tsc clean; **`contract build` failed** (21 refusals: `box`/`column`/`list`/`button` have no attribute `hook`; `spring(` is spelled `-exact-spring(`); **`cargo test -p t3-code-macos --lib` failed** (the bake in `build.rs`, same refusals); after the migration 2542 slots / 45 resources and 11/0; **macOS bundle build failed** (#234) | — | #234 (worked around locally) |
| 2 (2026-10-07) | `fac7ef1a1` + records | `bun test examples/t3-code` 2317 pass / 1 skip / 0 fail (2318 tests; 5 added); strict tsc clean; `contract build` 2546 slots / 46 resources; `cargo test -p t3-code-macos --lib` 11/0; AppKit binaries: see Checks; five checks: see Checks | images and record below | none for the adopted rows |

Checks (final source): `cargo build --all-targets --keep-going` ok; `cargo test --lib --bins --tests --no-fail-fast` 3377 passed, 0 failed, 33 ignored; `cargo clippy --all-targets --keep-going -- -D warnings` ok; `cargo fmt --all -- --check` ok; `git add -A && bun scripts/caps.mjs` within every budget (`app.contract` at 1,500 lines); `bun scripts/boot.mjs` ok. AppKit binaries (README recipe): 29 pass, 296 tests, 0 failures, 1 skipped (`ssh` live): activity 8 (1 added), notifications 4, menus 45 (1 added), contextmenu 19, sidebar 5, r12-sidebar 3, r8-keys 4, local-backend 44, terminal 38, transport 7 and the rest. Two do not run here, for this Mac's reasons, and no file they cover changed: `mermaid` needs `T3_SERVER` (a server with its web bundle; the lane's reference server has none), and `snapshot` aborts at its "actual current layout translates printable letter" precondition because the current input source is 2-Set Korean (`com.apple.keylayout.2SetHangul`). `timeline-keyboard` has its own recipe and was not run. The macOS bundles of base and branch build (the branch's with the `otool` shim, #234).

Lane (not committed, `target/lane-r5`): reference server `1e2ecbd975` (`apps/server/dist/bin.mjs serve`) on
127.0.0.1:16741 behind a trace proxy on 16742 that logs every client→server frame naming
`server.reportClientActivity` or `vcs.refreshStatus`. The server runs with an isolated HOME, CODEX_HOME,
CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME, `T3CODE_TELEMETRY_ENABLED=false` and a lane `SHELL` that keeps PATH as given
(the server reads PATH from a login shell, which put `/opt/homebrew/bin` ahead of the stubs). A first probe
therefore ran the real `codex` 0.151.0 with the lane's CODEX_HOME. `codex` is the reference's own
`codexCollabMockPeer.mjs` (copied: one model "GPT-5.5", user agent 0.160.1, `app-server --help` answers, one scripted
reply); `claude` is a stub that exits 1. Project `alpha` (a two-file git repository) was added with `t3 project add`.
Pairing tokens were minted with `t3 pair` and never printed.

Sessions (under `.realinput-lock`, 20:31–20:55). Agent sessions, one per build, each ran
`target/lane-r5/drive.mjs`. The before session ran with the base worktree's own `scripts/agent.mjs`. Attempts that
failed before any task flow are kept: before 1 (the proxy handed back a decoded body still marked `br`: "cannot
decode raw data" at pairing), before 2 and after 1–2 (no thread could be made: first the provider stub, then the
login-shell PATH). After 3 drew the thread menu at full window height: a closed submenu popover inside the menu took
its height under the agent. The submenus moved beside the menu (`fac7ef1a1`), and after 4 is the final session. Real
input: lane bundle copies `com.exact.t3code.laner5.before` / `.after` (ad hoc signed, `open -n`, addressed by pid).
Pairing went through accessibility (`set-value`, then a click and End, space, Backspace so the field heard input).
Both copies got a right-click on "First lane thread" and Copy was clicked. The lane Keychain item was deleted after
each copy, so no copy reads another's. The copies, their preference domains and data were deleted after. The user's
`com.exact.t3code.macos` domain hashed `f0919849` before and after. No 16742 Keychain item remains. Nothing listened
on 3773. A first cliclick click landed while another app (Electron) was frontmost and typed nothing; the rest went
through `orca computer` to the lane pid.

```
BEFORE (38352ceaf)  prefer has-focus …          refused by the base driver (the base framework has no such fact)
                    reports (proxy)              visible=true focused=false (twice; an agent window is never key)
                    tap row-<id> contextmenu     no thread-menu node; navigation.popover null (the module answers "dismissed")
                    Edit                         Undo Redo — Cut Copy Paste "Paste as Text" Delete — "Select All" — AutoFill
                                                 "Start Dictation…" "Emoji & Symbols" — Speech — "Toggle Diff ⌘D"
AFTER               has-focus true               page {visibilityState: visible, hasFocus: true}, notifications "unavailable (agent)"
                    has-focus false              page hasFocus false, notifications "unavailable (agent) (window inactive)",
                                                 report visible=true focused=false
                    has-focus back               vcs.refreshStatus, then report focused=true
                    tap row-<id> contextmenu     thread-menu with 14 rows; Snooze, Auto-settle behavior and Copy submenus
                    tap thread-menu-copy         the Copy submenu beside its row (Path, Branch, Thread ID)
                    snooze ▸ In 1 hour           the row moves to Snoozed; "Snoozed 1 thread, ⌘Z to undo"
                    Edit                         Undo Redo — Cut Copy Paste "Paste as Text" Delete — "Select All" — Speech —
                                                 AutoFill "Start Dictation…" "Emoji & Symbols"
REAL right-click    before and after             NSMenu: New thread on main, Pin, Settle, Snooze ▸, —, Rename, Regenerate title,
                                                 Mark unread, Filter by alpha, Auto-settle behavior ▸, —, Copy ▸ (Path, Branch,
                                                 Thread ID), Project settings, —, Archive thread, Delete (trash glyph)
```

## Evidence

| Scenario | Before (`38352ceaf`) | After |
| --- | --- | --- |
| Right-click a thread row under the agent | nothing opens | the row menu, Copy ▸ beside its row |
| Real right-click | the module's `NSMenu`, Copy ▸ open | the host's `NSMenu` from the popover; identical |
| Choose Snooze ▸ In 1 hour under the agent | not possible | the thread is snoozed |
| Composer command drawer edge | dark band at the bottom edge | clean edge |
| Focus under the agent (text) | `prefer has-focus` refused; reports `focused:false` | page fact, notifications and reports follow it; git refresh on focus |
| Edit menu (text) | Toggle Diff ⌘D shown at the end | the reference's items only, one Speech |

Images (pinned to the evidence commits):
- `https://raw.githubusercontent.com/ccheever/exact2/dfa835ac3c7a97b4c8ed0e8f3552e5b89ec49e68/adopt-main-fixes-r5/01-thread-menu-agent-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/31af13ec5006fb9d6a8e7c4e700c133cf005c58f/adopt-main-fixes-r5/02-thread-menu-native-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/8685283e020639d7c1487b367398fd70c387b56a/adopt-main-fixes-r5/03-thread-menu-choice-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/52f106b60c1c23f47db0fd2b1477129fc36d84e9/adopt-main-fixes-r5/04-drawer-backdrop-before-after.png`
- Record: `https://raw.githubusercontent.com/ccheever/exact2/32700857d7bf526b6d24a4be92f3b12fc09709b5/adopt-main-fixes-r5/05-drive-record.txt`

Findings for follow-ups (not this task):
- After adding the lane environment from Settings › Connections in a lane copy with no primary environment, the window
  showed "native.watch outside an answer" (`a5`, not committed; the code path is the fleet sync's, unchanged here;
  not tried on the base).
- `press` carries `MouseEvent` modifiers: the Send gesture and the sidebar's ⌘/⇧-click could read them instead of
  native monitors.

## Next action

Review the PR ([#236](https://github.com/ccheever/exact2/pull/236)). Not done, each with its blocker:
- **The bundle builds only with a local `otool` shim on main `261dd4e10`.** Blocker: #234.
- **Menus opened from the keyboard** at a focused row still use `T3Sidebar.swift`. Blocker: #235.
- **The Files tree's (Open with ▸) and the legacy sidebar project's right-click menus** are still the module's
  `NSMenu`s. They look like the reference's, but the agent cannot choose from them. Blocker: user decision (a
  follow-up task; this round's live-drive budget was spent on the sidebar menus #223 was filed for).
- **X27's remaining facts.** Blocker: user decision (no open issue tracks the rest of #113).
- **Notification click actions and the Dock badge** (#224), **nested backdrops and `saturate()`** (#225),
  **capture-phase and held-modifier keys** (#140), **an app-declared menu bar** (#141): upstream.

Not verified, each with its blocker:
- **A choice from the native `NSMenu` with real input.** #223 verified the host's press of a nested pick. Here the
  choice ran under the agent through the same row press, and the native menu was shown and opened. Blocker: user
  decision (another real-input session). Re-pairing a lane copy after its Keychain item is removed goes through
  Settings › Connections and adds a background environment, which shows no rows.
