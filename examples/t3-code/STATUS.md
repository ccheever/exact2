# T3 Code clone: status

Date: 2026-10-05. Reference: T3 Code `f870c419fc` (HEAD `1e2ecbd975`). exact2 base `c1522fdac`; checked against main `d2cb661eb`.

The newest clone source is in the mc-orch worktree (`~/Documents/work/0.projects/exact2-worktrees/mc-orch-e88043b25805/examples/t3-code/`, untracked, round 11 + an unfinished round 12). This directory holds an older copy (Oct 3) until the move. The full plan with item IDs is `~/.claude/plans/humble-humming-kay.md`. The framework-level list is in `EXACT2-GAPS.md`.

## 1. Done (round 11, verified by tests and agent drives against the web oracle)

Checks at round 11: `bun test` 991/0, strict `tsc` clean, `contract build` OK, 26 AppKit test binaries green, pixel matrix at 1280×840 and 840×620, light and dark.

- **Connection:** pairing, Keychain credentials, saved environments, reconnect ladder, SSH environments, outdated-host update, Load balancing / GitHub sharing settings (UI only).
- **Sidebar:** projects, shelves (Working/active/Settled/Snoozed), search, pin, settle and snooze, drag between shelves, row-action sweep, draft rows with context menu and undo.
- **Conversation:** history paging, streaming rows, work log, checkpoints, changed files, Markdown, code blocks, Mermaid, Shiki-matched code colours, workspace-preparation Retry.
- **Composer:** send/stop/queue/steer, model picker, reasoning and runtime options, Plan/Build, slash and @ menus, file/image/video attachments, stash, multi-model drafts.
- **Requests:** approvals and questions.
- **Version control:** branch picker, git actions, commit dialog, pull, publish wizard, new worktree, pull request checkout dialog.
- **Right panel:** workspace card, diffs, Files (tree, previews, editor, rendered HTML), linked pull requests, pull request surface (read and Ready/Merge), attachment previews (PDF, HTML, audio, video), Device surface (3D phone, tools, iPhone Duo, foldable).
- **Pages:** Pull Requests list and detail (read), Usage, welcome wizard, command palette, toasts, notifications.
- **Settings:** all 14 routes, theme editor and VS Code theme import, keybindings, providers (config only), connections, SnapShots, diagnostics, licenses.
- **Desktop shell:** reference menu bar, ⌘Q hold, window frame persistence, Korean 2-Set chord handling.

Round 12 (unfinished, in mc-orch): r12-sidebar and r12-render done (`bun test` 1025/0); r12-threads stopped mid-work; no native build or Swift tests after the stop.

## 2. To do (app level)

IDs follow the plan.

- **Round 12 wrap-up:** finish r12-threads; build and test the fixr edits; fix the real-input failures (scroll position after a wheel and a thread switch, Files editor focus, 1970 onboarding time, false "Some requests are slow" toast, `t3.server.origin` after removing a backend).
- **Move and rebase:** copy the newest clone here; rebase onto exact2 main (LLP 1091 `use` lines, new button layout, remove workarounds that main fixed).
- **A. Upstream (53 commits):** remove the reverted "/compact before resume" (A1); on-demand tool output `orchestration.getTurnItem` (A2); card-less tool rows (A3); descriptive file links (A4); Working order (A5); Shift PR quick actions (A6); multi-route environments (A7); install-aware update commands (A8); and A9–A17.
- **B. Large features:** terminal drawer and Terminal surface (Ghostty WASM in a WKWebView), floating device player drag/resize, interface font size (needs framework X3).
- **C. Pull requests:** comment and review composer, title/description/comment editing, reviewer and label pickers, Code tab with review threads, reactions, host stacks, PR-side thread links, hover cards, live refresh, Merge/Update branch header actions.
- **D. Providers and settings:** provider sign-in and runtime install, managed Codex with ChatGPT, ACP sign-in and session management, ChatGPT plan notices, reset credits, `/feedback`, thread Automations and live scheduled tasks, tracked project clone, custom model options, Update all.
- **E. Desktop:** embedded local T3 server, "This machine" (Local environment, Network access, Tailscale HTTPS, authorized clients, pairing links), `t3 app` activation, `t3code://` deep links, SSH password prompt, keep-awake, small menu and quit-hold differences.
- **G. Chat, composer, diff:** `server.reportClientActivity` every 25 s (G1, highest impact), diff and Files line comments, Cite from a reply, large-diff loading and expandable lines, "Delete the worktree too?", composer server-update banner, Auto balance, media actions, ultrathink frame and Cursor `fastMode:false`, legacy sidebar, compact composer menu, keybinding gaps.
- **Verification still owed:** real-input sessions, real device hub, reference Electron app as desktop oracle, protocol-trace comparison. GitHub: since 2026-10-07 the real-GitHub lane (`tools/github-lane`, task `20261007-real-github-lane`) replaces the fake `gh`: pull request reads and writes run against a disposable sandbox repository through the real `gh` the server spawns, signed in by the user (two accounts).

## 3. Framework level (exact2) — see `EXACT2-GAPS.md`

- **Policy decisions:** Browser surface (Chromium + CDP, X1), Developer Tools (X2), notification actions and badges (X28, #224), timers in data sources (X19).
- **Missing features:** app-settable root font size (X3), helper executables in the bundle (X4: on main since #215, not used until the embedded server is built), URL scheme delivery (X5), quit hold for modules (X6: `destroy()` at quit is on main since #200), resources outside the root file (X9), rich-text editing (X20), two-way WebSocket (X21), reactive layout facts (X22), scroll restore and offsets (X23), a capture-phase key handler and held-modifier state (X25, #140), an app-declared menu bar and keyboard-opened context menus (X26, #141, #235), title-bar options and a full-screen fact (X27; #113 closed without them), `app:/` video and PDF (X29), TS topic announce and pixel readback (X30).
- **Rendering parity:** text (X10), a backdrop beyond the parent's subtree (X11; `saturate()` is on main since #232, not adopted), textarea sizing (X12), hover during a pan (X13), non-Latin chords (X15), smart substitutions (X16), popover flips (X17), SVG morph (X18).
- **Already fixed on main:** `pointer-events: none` and overflow hit testing, `cursor`, popover top/bottom placement, key bubbling and `tabindex`, awaiting another answer's fetch, `title` tooltips; a watched topic's re-ask letting the reply in flight land (X14, #183: the app's read gate is removed) and the agent's middle and triple clicks, wheel at a point and held modifiers (X8, #186) (adopt-main-fixes-r3); `Intl.Locale` week data (X36, #204), module `destroy()` at quit (X6, #200; SSH's own observer removed), Chrome line breaks on macOS (X10 in part, #208), field paste/copy/cut events (X20 in part, #209), plain-scroll anchoring (X23d, #210) and a bundled PDF iframe (X29 in part, #205) (adopt-main-fixes-r4); the window-focus fact (X28 in part, #219: notifications, activity reports, git refresh and SnapShot settings on focus), `keyup`/`code`/`repeat` (X25 in part, #220: nothing removable yet), context-menu submenus (X26 in part, #223: the sidebar's menus are context popovers) and Edit ▸ Speech (#226), and Chrome's backdrop edges (X11 in part, #221) (adopt-main-fixes-r5). The same PR fixed a request storm: git status was re-read four times a second while a toast ticked. Status now comes from the stream, kept reads are shared in the transport, the 64-pending refusal is gone (the reference has none), and the module's menus no longer hold back the main queue. The macOS build of "T3 Code (Exact)" works with no shim since main #240 (#234), merged at `1f19b2400`.
