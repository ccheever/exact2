---
name: 20261007-adopt-main-fixes-r5
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: passed
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-r5
pr_url: https://github.com/ccheever/exact2/pull/236
verified_commit: 35197a341
---

# The clone adopts main's fixes, round 5 (#219, #220, #221, #223, #226; #113 checked)

## Outcome

The task branch records main `463acda68` in its ancestry and merges main `261dd4e10`, both as merge commits.
The PR must be merged with a merge commit, not squashed. Where a fix main merged covers what the clone
worked around, the workaround is gone and the clone behaves as T3 Code does. Where it does not, the issue files
name the open issue. The merge itself broke the clone: hooks were renamed access hatches, and LLP 1081 respells
names Exact invents. The clone now follows both. Main's linked-SDK check could not build an app named
"T3 Code (Exact)" ([#234](https://github.com/ccheever/exact2/issues/234), filed here). Main #240 fixed it, and the
branch then merged main `1f19b2400`: the bundle builds with no shim and the clone needed no change. `EXACT2-GAPS.md`,
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
[adopt-main-fixes-r4](20261007-adopt-main-fixes-r4.md) (#218),
[adopt-main-fixes-r3](20261007-adopt-main-fixes-r3.md) (#207).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| ancestry | main `463acda68` (round 4 was squash-merged) | `463acda68` | recorded with `git merge -s ours`, no file changed | `f6faefc2d` |
| merged main | origin/main into the task branch | `261dd4e10` | merge commit | `830eadecd`: `Cargo.lock` kept the `t3-code-macos` entry (Cargo then dropped `sys-locale`, which nothing uses), `QUEUE.md` kept both sides; no file under `examples/t3-code` changed |
| records | `feat(example)/t3-code` | `fbce02624` (#231 records sync) | merged before the record edits | `efd2ed4b6`, no conflict |
| records | `feat(example)/t3-code` | `f90277989` (#241 desktop parity audit) | merged before the storm-fix records | `f5a9e420d`, no conflict (documents only) |
| merged main | origin/main into the task branch | `1f19b2400` (#240 fixes #234; also #232) | merge commit | `35197a341`. `QUEUE.md` and `docs/agent-pitfalls.md` kept both sides (each side added an entry), and no file under `examples/t3-code` changed. Built with no `otool` shim. Clone checks and the five checks pass on it unchanged |

## Decisions per issue

| Issue | Result | Why |
| --- | --- | --- |
| merge breakage (61b33c2ca, 0bd99f606) | **fixed in the clone** | `hook=` → `hatch=` (76 attributes), app.json `hatches`, `ExactHatches`/`ExactHatchKey`/`ExactElement.hatch` in the module and its AppKit tests, `timeline.test.ts`; LLP 1081's script turned `spring(` into `-exact-spring(` (shell-morph, two rotations) and `tint-color` into `-exact-tint-color` |
| #234 (new) | **filed**, then **fixed by main #240 and adopted** (merged `1f19b2400`): the bundle builds with no shim, so no workaround is left. Before that, a local `otool` shim | `assertLinkedSdk` runs `otool -l "<app>/T3 Code (Exact)"`; otool reads `(Exact)` as an archive member and finds no SDK, so the build fails. `vtool -show-build` and `llvm-objdump` read the same file. The bundles here were built with a scratchpad `otool` that passes otool a symlink (not committed); renaming the app is not the clone's to do |
| #114 / X28 | **adopted** (issue stays open for #224) | Thread notifications choose toast or system notification by `page.hasFocus`. The activity reporter's `visible`/`focused` are the page's (`activityFacts`), held until known and reported on each change. The details card asks `vcs.refreshStatus` when focus returns (GitActionsControl's `focus` listener, which the clone had left out). SnapShot settings re-read on focus. `T3Notifications.active` and the reporter's AppKit window reads are gone; badge, click and sounds stay for #224 |
| #140 / X25 | **nothing to remove** (`closed-upstream`, #140 open) | Every monitor needs a window capture-phase handler or held-modifier state, or lives in a native view (terminal, device stream, composer) or the reference's own main-process code (held ⌘W, ⌘Q hold). Found: `press` already carries `MouseEvent` modifiers, which `T3ComposerIntent.take` and `T3Sidebar.pressModifiers` could use (not this round) |
| #141 / X26 (#223) | **adopted** for the sidebar | The thread row's and draft row's right-click menus are context popovers with nested submenu popovers. Each row carries its menu (`menuRows`; the bulk menu on a selected row), and a choice runs `menu-choice`/`draft-choice`. A real right-click shows the same `NSMenu`, and the agent can now open and choose the menu. Menus opened from the keyboard stay `T3Sidebar.swift`'s ([#235](https://github.com/ccheever/exact2/issues/235), filed here) |
| #141 / X26 (#226) | **adopted** | The clone's own Speech is removed (it would have shown twice). #226 also files the clone's ⇧⌘G and ⌘D buttons under Edit (Branch, Toggle Diff). `R8KeysMenus` hides Edit's app commands as File's, so Edit is DesktopApplicationMenu.ts's |
| #129 / X11 | **nothing to remove** (`closed-upstream`, #225 open) | The drawer's and dialogs' real blurs take #221 with no clone change. The flattened glass stays: nested backdrops and `saturate()` are #225 |
| "native.watch outside an answer" (follow-up) | **fixed in the clone** (only the round-5 build showed it; the cause is clone code present in both builds) | Real input, one session per build: only the round-5 build showed the transcript banner after a failed Add environment (record 06). `letGoAware` (`let-go.ts`) passed `watch()` through after its answer was let go. A refresh let go inside one of `fleet.sync`'s tolerated reads went on to `readLocalBackend`, whose `native.watch('t3.local')` the prelude refuses outside an answer, and the refresh's catch wrote that Error into `client.error`. The watch now rejects as `superseded` once the answer is let go (`cf7b15baf`; `let-go.test.ts` reproduces the message through `client.refresh`). The path and the race are the same on the base. Why only the round-5 build showed it is a hypothesis: its page-fact answers (shellView, the details card, SnapShot settings) are asked again on each window activation, which may shift when the snapshot answer is let go. With one session per build, and an uncontrolled extra step in the after session (Snooze before its quit), chance explains it as well. No main commit introduced it (independent review: the prelude diff only adds database tracking, and `forget_calls` is unchanged); nothing was filed. Final session: no banner on `da3aae081` (record 07) |
| Request storm (a): `vcs.refreshStatus` four times a second | **fixed in the clone** (2525124ca, 3373f7919, 3e939ecaa, e4087409d) | While a toast is up the window asks the strip (`composerBranches`) and the details card again every 500 ms (app.contract `shellTick`), and Exact lets the previous answers go. The strip's status read and the card's branch row read were kept only after a reply. A reply slower than the tick (refreshStatus can wait on a remote fetch) was therefore asked for on every tick, on the base as on this branch. The reference reads status from the subscribeVcsStatus stream (BranchToolbarBranchSelector, GitActionsControl), asks refreshStatus only on window focus (debounced 250 ms), on visibility and when a git menu opens, and shares one listRefs request per input (`listRefsFamily`). Now the strip and the card read the stream, which follows the strip's workspace while the card is closed. A kept read is a shared read: T3Transport answers an identical read still pending with the same reply, and a write naming a cwd ends the sharing for that cwd. A never-seen checkout assumes Git, as in ChatView. After a branch switch the stream resubscribes and the strip keeps the switched name until it reports. A first attempt resent on a 3 s clock (3373f7919); review 2 failed it, because the clock domains were mixed and the clock stops with the tick, and 3e939ecaa replaced it |
| Request storm (b): nothing sent while a native menu is open | **framework, reported (not filed)**; the clone's own menus fixed (3373f7919) | ExactKit's `MenusMac.context()` (and `show()` for button menus) calls `menu.popUp` inside `DispatchQueue.main.async`. CoreFoundation does not drain the main queue in a run loop nested in its own callout, and ExactKit hands every `native.later` call to the module through that queue (`NativeModule.swift` `nativeLaterCallback`), so the calls wait until the menu closes. `.common` timers keep the runner ticking meanwhile. Exact repro (standalone, `menu-repro` in the session scratchpad): a menu opened inside `DispatchQueue.main.async` ran 0 of about 20 main-queue blocks posted during 2 s of tracking (NSEventTrackingRunLoopMode), while `.common` timers fired; the same menu opened from `CFRunLoopPerformBlock` in the common modes ran all of them. T3Transport is not at fault: its sends run on its own queue. The clone's own menus (keyboard thread menu, Files/PR, media, terminal) now open from a common-modes run-loop turn (`T3MenuTurn`) |
| Shared reply over 512 KB (coordinator follow-up) | **fixed in the clone** (60ada7142) | A reply over 512 KB comes back as a native transfer that its reader releases once read (`protocol.ts` bridgeReply). Joined callers shared one finished reply, so the second read a transfer the first had released ("The pending server response was released."). Joined callers are now kept as a list, and each is finished on its own with its own transfer: on the reply, on a timeout and on retire. XCTest `testJoinedCallersEachReadTheirOwnLargeReply`: two joined callers, a 600 KB reply, each reads its transfer to the end and releases it. On `1ae9fab49` the second caller's chunk 7 is stale |
| 64-pending refusal | **removed** (2525124ca) | The reference's RPC client (packages/client-runtime/src/rpc/client.ts) has no cap on pending requests, so a user's snooze queued behind reads still goes out. Each pending request still ends at its deadline (30 s at most), and the outbox and 16-stream limits stay |
| #113 / X27 | **re-checked: still missing** | No `titlebar-area-*`, title-bar setting or full-screen fact on `261dd4e10`. No open issue tracks the rest (user decision) |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Clone builds on the merged base | — | `bun test examples/t3-code`, strict tsc (ES2023), `contract build`, `cargo test -p t3-code-macos --lib`, AppKit binaries, caps | green | macOS | numbers below |
| Focus is the page's | lane (below), agent | `prefer has-focus true/false/true`; `state`; trace proxy | `page.hasFocus` and `shell.notifications` follow; a report with `focused` follows each change; a git refresh when focus returns | macOS 1280×840 | record 05 |
| Row menu with a submenu | lane, two threads | `tap row-<id> contextmenu`, `tap thread-menu-copy`, `tap thread-menu-snooze`, `tap thread-menu-snooze:hour` | the menu and its submenu beside the row; the thread snoozes | macOS 1280×840 | images 01, 03 |
| Native menu | lane bundle copies (own bundle ids), real right-click | `orca computer click … --mouse-button right`, then Copy | the same `NSMenu` before and after, Copy ▸ open | macOS | image 02 |
| Edit menu | agent `state.menus` | — | Undo, Redo, —, Cut, Copy, Paste, Paste as Text, Delete, —, Select All, —, Speech (+ AppKit's) | macOS | record 05 |
| No banner after a failed Add environment | lane copy with no saved credential (Keychain item and preferences removed, data kept) | real input: Settings › Connections › Add environment › Remote link, an unreachable host, Add environment | the dialog's "Could not connect to the server." only; no transcript banner | macOS | record 06 (the regression); `let-go.test.ts`; record 07: **passed** on `da3aae081` |
| Native menu choice | lane copy, seeded threads | real right-click on the row, Snooze ▸, a real click on "In 1 hour" at its screenshot position; the tree; un-snooze | the thread is snoozed about 1 h ahead | macOS | record 07, image 07: failed on `da3aae081` (the snooze refused); record 08, image 08: **passed** on `e4087409d` (menu held 11 s; snoozedUntil one hour after the click; Wake thread clears it) |
| Request cadence | lane, the server-update notice up (the 500 ms tick running) | the proxy logs every request's method for the whole session | no request repeated per tick; refreshStatus only at startup, focus or a git menu | macOS | record 08: one refreshStatus, one subscribeVcsStatus, one listRefs in about 2.5 min |
| Backdrop edge | lane thread | `type composer "/"` | the drawer's edge as Chrome's | macOS | image 04 |

## Progress

Implemented 2026-10-07. Round-5 follow-up (coordinator, 2026-10-07): the "native.watch outside an answer" finding was
tried on both builds, found to be a regression of this round in clone code, and fixed; the native menu was opened with
real input. Final session (coordinator-approved, one after-build session on `da3aae081`, no retry): the fix **passed**
(no banner; the dialog's error only), and the native menu choice **failed**. The real click on "In 1 hour" reached the
app, but the snooze was refused: "Failed to snooze thread / Too many server requests are already pending." No thread
was snoozed. The coordinator then had the request storm fixed in this PR (a: the repeated status read, b: the menu
stall, and the 64-pending cap), with one real-input session on the after build: the native menu choice **passed**
(record 08). Two more changes followed on the coordinator's word: each joined caller of a shared read gets its own native
transfer (60ada7142), and main `1f19b2400` was merged (#240 fixes #234, so the bundle builds with no shim; no clone
change needed). Verification: passed. Every check that runs here passes on `35197a341`, the merge that holds both (runner
attempt 8, `source_unchanged: true`, source digest `f2b0db9f`:
[`attempt8-report.json`](../../evidence/20261007-adopt-main-fixes-r5/attempt8-report.json); attempts 1-7 are kept; recipe and
`swift-tests.sh` beside them). The live checks were run by hand on `e4087409d` (records 07 and 08), before the transfer fix
and the main merge; those two are covered by tests and builds, not by another session.
Independent reviews: [`review.md`](../../evidence/20261007-adopt-main-fixes-r5/review.md) (the watch fix PASS; the storm fix
PASS, then FAIL on 3373f7919, then PASS on the shared reads). The task stays under `tasks/` for the coordinator's records
sync; the PR stays a draft.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | `830eadecd` (merged base, no clone change) | `bun test examples/t3-code` 2312 pass / 1 skip / 0 fail; strict tsc clean; **`contract build` failed** (21 refusals: `box`/`column`/`list`/`button` have no attribute `hook`; `spring(` is spelled `-exact-spring(`); **`cargo test -p t3-code-macos --lib` failed** (the bake in `build.rs`, same refusals); after the migration 2542 slots / 45 resources and 11/0; **macOS bundle build failed** (#234) | — | #234 (worked around locally) |
| 6 (2026-10-08, transfer and main) | `60ada7142` (transfer fix); `35197a341` (main `1f19b2400`) | The XCTest above fails on `1ae9fab49` and passes now. Main merged with no clone change. The bundle builds with no `otool` shim. `bun test examples/t3-code` 2323 pass / 1 skip / 0 fail; strict tsc clean; `contract build` 2546 slots / 46 resources; `cargo test -p t3-code-macos --lib` 11/0; AppKit binaries 29 pass (mermaid and snapshot as before); five checks: see below; caps ok. No live session (not needed for these) | runner attempts 7–8 | none |
| 5 (2026-10-08, storm fix) | `e4087409d` (bundle built with the `otool` shim) | Tests that fail before and pass after: `vcs-status-cadence.test.ts` (5 status reads in 2 s and 9 with the card open before, at most one now; the picker leaves its loading state when the shared reply reaches the answer that joined it, which fails on 3373f7919), XCTests `testPendingRequestsHaveNoCapAsInTheReference` (17 refusals before) and `testSharedReadsJoinAnIdenticalPendingOne`. `bun test examples/t3-code` 2323 pass / 1 skip / 0 fail; strict tsc clean; `contract build` 2546 slots / 46 resources; `cargo test -p t3-code-macos --lib` 11/0; AppKit binaries 29 pass (mermaid and snapshot as before); five checks green (3377 passed, 0 failed, 33 ignored); caps ok. One real-input session under `.realinput-lock` (00:29:57-00:32:45, no retry): real right-click on "First lane thread", Snooze ▸ open 11 s, a real click on "In 1 hour (1:30 AM)" at window point (410, 308): "Snoozed 1 thread, ⌘Z to undo", the row in Snoozed, the server's snoozedUntil 16:31:12.913Z for a click at 15:31:11.841Z; Wake thread (real right-click, element click) emptied it. The proxy's full request log: one refreshStatus, one listRefs, nothing per tick, activity reports kept their 25 s cadence while the second menu was open | record 08; image 08 | none |
| 4 (2026-10-07, final session) | `da3aae081` (bundle built with the `otool` shim) | One real-input session under `.realinput-lock` (21:56–22:00), no retry. Item 1: after Add environment the tree has "Could not connect to the server." (dialog and page) and no "native.watch outside an answer", also after Back: **passed**. Item 2: real right-click on "First lane thread", Snooze ▸ (element), then "In 1 hour (10:57 PM)" clicked at window point (410, 308) from the screenshot. The app showed "Failed to snooze thread / Too many server requests are already pending." The six rows were unchanged, so there was nothing to un-snooze: **failed**. The menu was open 17 s while the screenshot was read. In that time no frame left the proxy, and 64 `vcs.refreshStatus` went out within 4 ms after it closed. The client was sending `vcs.refreshStatus` at 4 per second (2 per 500 ms), the rate the base showed too, so the transport's 64-pending cap (`T3Transport.swift`) refused the snooze | record 07; image 07 | the repeated `vcs.refreshStatus` (Next action) |
| 3 (2026-10-07, follow-up) | `364f5baa6` (sessions); `cf7b15baf` (fix) | Real input, one session per build under `.realinput-lock` (21:26–21:28): before no banner, after "native.watch outside an answer" (record 06); the after session's real right-click opened the native menu and its Snooze submenu, but no submenu item could be chosen (the accessibility tree lists only the top-level items). Fix `cf7b15baf`: `bun test examples/t3-code` 2318 pass / 1 skip / 0 fail (1 added, and the seam test checks `watch()`); strict tsc clean; `contract build` 2546 slots / 46 resources; `cargo test -p t3-code-macos --lib` 11/0; five checks: see Checks | record 06; runner attempts 1–2; review | the two live checks (Next action) |
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
| Failed Add environment, real input (text) | the dialog's error only | the same, plus the transcript banner "native.watch outside an answer"; on `da3aae081` (with `cf7b15baf`) the dialog's error only again (record 07) |
| Snooze ▸ In 1 hour from the native menu, real input (images 07, 08) | — (the real click was not tried on the base) | on `da3aae081` "Failed to snooze thread / Too many server requests are already pending."; on `e4087409d`, after 11 s with the menu open, "Snoozed 1 thread" and the row in Snoozed |

Images (pinned to the evidence commits):
- `https://raw.githubusercontent.com/ccheever/exact2/dfa835ac3c7a97b4c8ed0e8f3552e5b89ec49e68/adopt-main-fixes-r5/01-thread-menu-agent-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/31af13ec5006fb9d6a8e7c4e700c133cf005c58f/adopt-main-fixes-r5/02-thread-menu-native-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/8685283e020639d7c1487b367398fd70c387b56a/adopt-main-fixes-r5/03-thread-menu-choice-before-after.png`
- `https://raw.githubusercontent.com/ccheever/exact2/52f106b60c1c23f47db0fd2b1477129fc36d84e9/adopt-main-fixes-r5/04-drawer-backdrop-before-after.png`
- Record: `https://raw.githubusercontent.com/ccheever/exact2/32700857d7bf526b6d24a4be92f3b12fc09709b5/adopt-main-fixes-r5/05-drive-record.txt`
- Follow-up record (text: the banner is behind the Settings backdrop, so the screenshots do not show it; the
  accessibility tree does): `https://raw.githubusercontent.com/ccheever/exact2/ee2b4323bf709761f42b504d42e7b2944db267e1/adopt-main-fixes-r5/06-add-environment-record.txt`
- Final session record (both items, the proxy's `vcs.refreshStatus` counts on the base and on `da3aae081`):
  `https://raw.githubusercontent.com/ccheever/exact2/74a652d28b058f0960451ee01050e07b9fe78870/adopt-main-fixes-r5/07-final-session-record.txt`
- Final session, the native menu choice (the open submenu before the real click; the refusal after it):
  `https://raw.githubusercontent.com/ccheever/exact2/e28711837f243b938bfd8a3c9585eb07342c10fe/adopt-main-fixes-r5/07-native-menu-choice-real-input.png`
- Storm-fix session record (the session log, the server's snooze fields, every request the client sent):
  `https://raw.githubusercontent.com/ccheever/exact2/89474e644b2b8e627b00727eff537e8804180c32/adopt-main-fixes-r5/08-storm-session-record.txt`
- Storm-fix session, the native menu choice (Snooze ▸ open 11 s before the real click; the thread snoozed after it):
  `https://raw.githubusercontent.com/ccheever/exact2/77840e00153c4a5603de230922c4499e0d1048f5/adopt-main-fixes-r5/08-native-menu-snooze-real-input.png`

Follow-up sessions (coordinator's round-5 follow-up; `realsession.sh` in the session scratchpad, not committed). The
same lane and the same steps for each build, one session each: pair a lane copy through the wizard, quit, delete its
Keychain item and preferences domain (data kept), relaunch, Open Connections, Add environment, Remote link, the Host
field set and nudged (End, space, Backspace left `http://127.0.0.1:1674`, unreachable, in both), a pairing code, Add
environment. The after session also right-clicked "First lane thread" and opened Snooze ▸ before its quit. The
copies, their Keychain items, preference domains and data were deleted after; the lane server and proxy were stopped
by their recorded pids.

Final session (coordinator-approved, `final-lib.sh` in the session scratchpad, not committed). The lane was restarted with
its recorded pids, the same environment and the same steps as the follow-up, on a copy of the `da3aae081` bundle
(`com.exact.t3code.laner5.final`). Afterwards the copy, its preferences, data and caches were deleted, no lane Keychain
item remained, the lane server and proxy were stopped by their recorded pids, and the lock was released.

Storm-fix session (coordinator-approved, `storm-lib.sh` in the session scratchpad, not committed): the same lane on a copy
of the `e4087409d` bundle (`com.exact.t3code.laner5.storm`), the proxy logging every request's method. The lock was taken
once the provider sign-in lane released it. The thread was un-snoozed with Wake thread, so the lane's threads are as
found. Afterwards the copy, its preferences, data and caches were deleted, no lane Keychain item remained, the lane
server and proxy were stopped by their recorded pids, and the lock was released. While waiting for the lock earlier, a
check of mine removed the other lane's lock by mistake at 22:30:10 (its `mkdir` failed and the cleanup still ran). I
restored it at once with a note, and since then the helper releases only a lock whose note is its own.

Findings for follow-ups (not this task):
- Resolved in the follow-up: "native.watch outside an answer" (Decisions per issue); passed on a build in the final session.
- Resolved in the storm fix: the repeated `vcs.refreshStatus` and the 64-pending refusal (Decisions per issue). The
  shared large reply, a review finding at first, was fixed after (60ada7142).
- Reported, not filed (framework): ExactKit's `MenusMac` opens context and button menus inside `DispatchQueue.main.async`,
  so no `native.later` call reaches a module while one is open (Decisions per issue, with the repro). With the clone's
  reads no longer repeated per tick, nothing piles up behind a menu here, and the reported cadence is unaffected.
- From the storm reviews, not fixed here: `runModal` calls inside main-queue blocks (`T3ContextMenu.swift` save panels,
  `T3PanelsNative.swift`, `R6DeviceStream.swift`) stall the main queue the same way while open. A shared reply over 512 KB
  handed every joined caller one native transfer, which the first reader released (fixed in 60ada7142). A never-seen non-Git checkout shows
  the assumed-Git strip until its status arrives, as ChatView does.
- From the follow-up's independent review ([`review.md`](../../evidence/20261007-adopt-main-fixes-r5/review.md)), none of
  them observed. Other let-go paths still show something or lose state. `auto-balance.ts` `retargetDraft` toasts "Could
  not switch machine" for a let-go. `settings-b-outdated.ts` `probeDescriptors` caches a let-go probe as no descriptor
  until the page closes. Prelude calls outside `native` (crypto, a plain `fetch`) throw a plain "outside an answer"
  error after a let-go. And `letGo` reads only the kind, so the runtime's two Aborted failures that are not a let-go
  (a panicked native call, an owner that ended without a reply) are hidden as let-go for `later()` and now `watch()`.
- `press` carries `MouseEvent` modifiers: the Send gesture and the sidebar's ⌘/⇧-click could read them instead of
  native monitors.

## Next action

Review the PR ([#236](https://github.com/ccheever/exact2/pull/236)). Not done, each with its blocker:
- **Menus opened from the keyboard** at a focused row still use `T3Sidebar.swift`. Blocker: #235.
- **The Files tree's (Open with ▸) and the legacy sidebar project's right-click menus** are still the module's
  `NSMenu`s. They look like the reference's, but the agent cannot choose from them. Blocker: user decision (a
  follow-up task; this round's live-drive budget was spent on the sidebar menus #223 was filed for).
- **X27's remaining facts.** Blocker: user decision (no open issue tracks the rest of #113).
- **Notification click actions and the Dock badge** (#224), **capture-phase and held-modifier keys** (#140), **an
  app-declared menu bar** (#141): upstream.
- **Glass beyond the parent's subtree and `saturate()`** (X11). #225 closed when main #232 merged (`saturate()` beside
  `blur()`, now in this branch), but #232 says cross-parent sampling on macOS stays open, and no open issue tracks it. The
  flattened glass stays. Blocker: user decision (adopt `saturate()` where the parent's subtree suffices, and whether the
  rest becomes an issue).

Nothing is left unverified. The native menu choice with real input passed in the storm-fix session (record 08), and the
banner fix passed in the final session (record 07).
