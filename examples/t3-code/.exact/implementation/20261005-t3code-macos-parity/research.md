# Research for 20261005-t3code-macos-parity

Sources: six read-only product investigations on 2026-10-05 (round 12 + upstream; chat,
composer and keys; pull requests and review; providers and settings; desktop and local
server; terminal, player and font size), one library feasibility investigation, the clone's
own docs, and an earlier draft gap inventory (IDs A1–H; a local planning note, not needed to
use this plan: every item it listed is in the tables below with its status). Reference: T3 Code `1e2ecbd975` (local HEAD; no fetch was made). Clone: the newest
tree in the mc-orch worktree (untracked; line numbers from 2026-10-05). No framework source
was read for this plan; framework facts below come from the bundled library or from the
clone's own `EXACT2-GAPS.md` (written by earlier sessions, labelled as such).

Status words: **missing** (no code), **partial** (some code; the ticket says what is left),
**done** (no ticket), **excluded** (spec exclusion).

## Product observations

### Round 12 and upstream `f870c419fc..1e2ecbd975`

| Workflow | Observed behavior (reference) | Source | Clone status | Edge cases | Uncertainty |
| --- | --- | --- | --- | --- | --- |
| No-project draft switches machine (R12 T1) | Run-on lists machines with a "No project" folder; `projects.ensureScratch`; 10 s wait; "Preparing machine"; toast "Could not switch machine" | `ChatView.tsx:2662-2730,4348-4420`; `projectCommands.ts:56-62,124-150` | partial — last drive hit the server's `ScratchFolderError` | started thread keeps its machine | why lane B's folder creation failed |
| Worktree card hides after start (R12 T2) | hides once the activity run has `startedAt` | `ChatView.tsx:3913-3917` | **done** | held queued run is not the activity run | — |
| Device session floats on load (R12 T3) | floats unless sheet layout ≤ 980 pt | `ChatView.tsx:5285-5338` | partial | first snapshot as baseline | which rule the reference follows on load |
| Tab strip at 840 (R12 T4) | scrolls; arrow buttons; active tab in view | `RightPanelTabs.tsx:804-830,1317-1346` | partial | step max(120, 75%) | verdict of the last pair |
| Non-Git machine strip (R12 T5) | strip with only "Run on" | `BranchToolbar.logic.ts:67-79` | partial (never driven, no test) | `hostsRestingComposerControls` | — |
| F1 wheel + thread switch | scroll row/offset restored per thread | `timelineScrollAnchoring.ts:164-175` | partial (fix newer than its test run) | jump ends the settle tail | real-wheel recheck |
| F2 Files editor click below last line | editor gets the text | `FilePreviewPanel.tsx:599` | partial (fix unbuilt) | — | reference caret position |
| F3 onboarding time | wall-clock ISO | `onboarding/firstRun.ts:13` | missing (stores 1970) | other persisted wall times | root cause |
| F4 slow-request toast | 15 s per request (120 s for update methods), cleared by the reply | `requestLatencyState.ts:77-112` | missing | toast twice after A off / B on | cause |
| F5 removed backend origin | Remove forgets pairing, credentials, cached threads | `ConnectionsSettings.tsx:2600-2640` | missing (key is in `T3Transport.swift`) | — | — |
| A1 compact before resume | removed upstream | `2188bdd8b5` | clone still sends `/compact` | keep `cc:compact` | — |
| A2/A3 tool output | `orchestration.getTurnItem`; four output states; card-less rows; red `exit N` only if ≠ 0 | `itemDetail.ts:82-207`; `V2ItemInspector.tsx:99-190` | missing | 256 KB cap; Read/skill output withheld by newer servers (N1) | — |
| A4 descriptive file links | prose label + chip | `markdownLinks.ts:269` | missing | — | Markdown parser behavior |
| A5 Working order | `latestUserAuthoredMessageAt` | `threadInbox.ts:74` | missing | fallback `latestRun.requestedAt` | — |
| A6 Shift quick actions on PR rows | Close/Merge/Ready/Reopen; "Merge requested" | `PullRequestSpeedActions.tsx:24-135` | missing | stacked PR disabled | live modifier state |
| A7 multi-route environments | ordered routes, learned routes, fallback, cooldown | `client-runtime/src/connection/routes.ts` | missing (one origin per entry; duplicate rows) | route to another machine refused | — |
| A8 install-aware update command | npm-global vs relaunch | `versionSkew.ts:123` | partial | remote servers only | — |
| A9–A16 small UI | provider update popover from list icon; slash-command retry; simulator diagnostics at step 0; pin-off icon; Azure DevOps mark; `composer.cycleHost` (no default key); subagent tooltip "model · account"; popover z-order | commit table in investigation | A9/A11/A15 partial; others missing; A16 unknown | Azure mark needs CC BY 4.0 attribution | A16/A17 need a drive |
| A17 scroll closes tooltips | real timeline scroll closes hovered tooltip and PR preview | `tooltip.tsx:16-80` | unverified | focused tooltip stays | hover events during a pan (X13) |
| `c8d7d50a73` PR Link to thread | picker for an unsent draft | `PullRequestThreadLinks.tsx:53-157` | missing (new) | — | — |
| `toolSource`/`toolIcon` | rendered for more providers | `ec20db4a5a` | missing (predates range) | — | — |

Commits needing no client change: web/Windows-only, Browser-only (`2dfef8779e`, excluded),
server-only (carried by the embedded runtime pin), mobile/infra.

### Chat, composer and keys

| Workflow | Observed behavior | Source | Clone status | Edge cases | Uncertainty |
| --- | --- | --- | --- | --- | --- |
| G1 client activity report | `server.reportClientActivity` to every env: 250 ms debounce; every 25 s; focus/blur/visibility/online/env change/scope change; interactions only if > 45 s since last; TTL 45 s; scopes provider-status, vcs-status{cwd}, diagnostics | `lib/backgroundActivityReporter.ts:22-250`; `server/src/background/BackgroundPolicy.ts` | missing (largest single gap: server stops provider, usage and VCS/PR refresh without it) | server clamps TTL 1–120 s | — |
| G5 delete worktree too | orphan-worktree confirm, `vcs.removeWorktree{force}` | `useThreadActions.ts:422-600`; `worktreeCleanup.ts` | missing | archived thread: no prompt | `force` drops uncommitted work |
| G6 server update banner | composer + details banner with progress/fail/retry | `ServerUpdateAction.tsx`; `ComposerServerUpdateStatus.tsx` | missing (Settings one-shot only) | 2 s offline grace (N2); "Client and server versions differ" (N3) | — |
| G7 Auto balance | `server.getHostResources`, scoring, multi-machine update banner | `useLoadBalancedEnvironment.ts`; `client-runtime/src/load-balancing.ts` | missing (settings only) | manual pick wins | scope permissions |
| G8 media actions | context menu, copy/save, failure fallbacks | `components/media/*` | missing | 64 M-pixel copy limit | blob download path in Electron |
| G9 ultrathink / Cursor fastMode | rainbow frame, prefix rewrite, implicit `fastMode:false` | `composerProviderState.tsx:50-160` | missing | slash commands skip prefix | — |
| G10 legacy sidebar | switch swaps in `LegacySidebar` (3,825 lines) and changes `chat.new` | `LegacySidebar.tsx` | partial (stored only) | device-level | full parity not read |
| G11 compact composer menu | overflow "More composer controls" | `CompactComposerControlsMenu.tsx`; `composerFooterLayout.ts:184-224` | missing | 1 px slack to grow back | — |
| G12 queued edit attachments; inline `$skill` chips | attachments in `queued-run.edit`; `$name` matching | `queuedMessageEdit.ts`; `SkillInlineText.tsx` | partial / partial | — | — |
| G14 keys | ⇧⌘↩ steer, ⌥↑ edit queued, ⇧⌘H host, `composer.cycleHost`, `rightPanel.toggleMaximized`; ⇧⌘[ ] must not wrap (clone wraps) | `packages/shared/src/keybindings.ts:21-111`; `ChatView.tsx:7612-7830` | missing / partial | key repeat ignored | — |
| G15 small items | multi-env usage refresh, approval warning tooltip, 20 editor icons, "No active thread" | various | partial / missing | — | three draft items could not be mapped |

### Pull requests and review

| Workflow | Observed behavior | Source | Clone status | Edge cases | Uncertainty |
| --- | --- | --- | --- | --- | --- |
| C1 comment / review composer | Comment, Close/Reopen with comment, Review verdicts + line comments | `PullRequestComposer.tsx`; `pullRequests.comment`, `.submitReview` | missing | author gets only "comment" verdict | — |
| C2 editing | title inline; description/comments Write/Preview; collapse > 240 px | `pullRequestEditing.logic.ts` | missing (`canEdit` logic wrong) | own comments only | — |
| C3 reviewers/labels pickers | candidates on open, search, toggle | `PullRequestCandidatePicker.tsx` | partial (logic only) | permission tooltip | — |
| C4 Code tab | scope menu, stacked/split, Viewed, threads Reply/Resolve/Edit, Fix in a thread | `PullRequestCodeTab.tsx` | missing | > 300 files fallback | reuse of `diff.ts` |
| C5 activity | separate read, error + Retry, review comments and threads | `PullRequestActivityUnavailableState.tsx` | partial and wrong (false "No comments yet.") | `commentsTruncated` | — |
| C6 reactions | eight emoji pills, optimistic | `PullRequestReactions.tsx` | missing | description reactions not rendered by the reference either | — |
| C7 skeletons | detail/timeline ghosts, cached snapshot | `PullRequestGhosts.tsx` | partial (list ghost done) | — | — |
| C8 stacks | n/m menu, Merge/Rebase stack | `PullRequestStackMenu.tsx`; `githubStackActions.ts` | partial | stale heads error | — |
| C9/C10 links and previews | linked threads, `#N` autolinks, 350 ms hover card | `PullRequestThreadLinks.tsx`; `PullRequestLinkPreview.tsx` | partial / missing | hover during a pan (X13) | — |
| C12 routing | PR reads/writes via another env with the same GitHub account | `pullRequestRouting.ts` | missing effect | — | multi-environment client design |
| C13 live refresh | `pullRequests.subscribeRefreshes` + timers | `useLiveRefresh.ts` | partial | idle after 6 min | — |
| C14 header actions | primary control, More menu, update branch, five dialogs | `PullRequestDetailPanel.tsx` | partial (Close has no confirm; 6 of 10 actions) | merge-method resolution | — |
| G2 line comments | gutter + / drag on diffs and Files preview | `AnnotatableCodeView.tsx`; `reviewCommentContext.ts` | missing | one draft at a time | — |
| G3 Cite | selection toolbar, citation comment | `AssistantSelectionToolbar.tsx` | partial | UTF-16 anchors | native transcript selection offsets |
| G4 large diffs | per-file loading, Retry, expandable unmodified lines | `DiffPanel.tsx:394-495`; `review.getDiffFileContents` | missing | caps 120,000 B / 1 MiB | — |
| G13 diff view | folder tree, sticky headers, no word emphasis (fixed `"none"`, not a toggle) | `DiffFileTree.tsx`; `StyledDiffCodeView.tsx:79` | differs | — | sticky rows in lists |
| Fake gh | serves reads only; viewer is author + ADMIN on every PR; no stdin logging | lane `r6-pr/fakegh/gh.mjs` | partial (apparatus) | approve/request-changes unreachable | — |

### Providers and settings

| Workflow | Observed behavior | Source | Clone status | Edge cases | Uncertainty |
| --- | --- | --- | --- | --- | --- |
| D1 provider sign-in | account row, seven phases, method picker, browser/credentials/terminal/device-code | `ProviderAuthenticationSection.tsx`; `provider.auth.*` | missing | read-only disables; paste-URL form | — |
| D3 runtime install | install/update/reinstall/cancel/remove with progress | `ProviderSetupSection.tsx`; `provider.install.*` | missing | custom binary path | — |
| D4 Managed Codex + ChatGPT | client starts install + sign-in; server owns OAuth; **desktop main listens on loopback** 127.0.0.1:49152–65535 for 300 s | `CodexSetupSection.tsx:397-545`; `apps/desktop/src/ipc/methods/providerAuth.ts` | missing (clone comment "server runs sign-in" is wrong) | port in use; expiry | needs a real ChatGPT account |
| D5 ACP wizard sign-in | sign-in step, registry icons | `ProviderWizardAuthenticationStep.tsx` | partial | icon allow-list | — |
| D6 ACP sessions | native sessions, log out, agent providers, URL auth | `AcpSessionManagementSection.tsx` | missing | — | — |
| D7 ChatGPT plan notices | one-time dialog, footer, usage row | `ChatGptWelcomeCoordinator.tsx` | missing | usage-limit banner unreachable at HEAD | — |
| D8 reset credits / D9 `/feedback` | use reset credit; upload feedback | `UsageLimits.tsx`; `threadFeedback.ts` | missing | — | — |
| D10 automations / D11 tracked clone | `scheduledTasks.subscribe` + thread panel; `projectClone.*` toasts | `ScheduledTasksSettings.tsx`; `ProjectCloneToastCoordinator.tsx` | partial / partial | 16-stream cap | — |
| D12 Update all providers | header button for every candidate | `ProviderUpdatesAction.tsx` | missing | — | — |
| D13 custom model editor | option rows, presets, validation | `CustomModelEditor.tsx` | partial | — | — |
| D14 redaction / D15 mixed switch / D16 theme editor host | blurred email; `aria-checked="mixed"`; floating editor that survives navigation | `RedactedSensitiveText.tsx`; `ScopedSwitch.tsx`; `ThemeEditorHost.tsx` | partial each | mixed switch click turns off everywhere (clone bug) | hit-testing app colors |
| Extra | status banner setup button; model-picker setup footer; update toast states; instance id `codex_<uuid>` | various | missing | handoff stream must not replay | — |

### Desktop and local server

| Workflow | Observed behavior | Source | Clone status | Edge cases | Uncertainty |
| --- | --- | --- | --- | --- | --- |
| E1 launch | Electron as Node runs `app.asar/apps/server/dist/bin.mjs --bootstrap-fd 3`; port from 3773 upward (bind on 127.0.0.1, 0.0.0.0, ::); login-shell PATH probe; 24-byte token; readiness poll in 60 s rounds; `POST /oauth/token` (all 8 scopes); restart `min(500 ms·2^n, 10 s)`; SIGTERM → SIGKILL 2 s; quit cap 5 s; `server-child.log` on failure | `apps/desktop/src/backend/*`, `app/DesktopApp.ts`, `shell/DesktopShellEnvironment.ts` | missing | port changes per launch | orphan after crash |
| E1 source (self-contained build) | Official plain-Node form: CLI archive `t3-<ver>-darwin-arm64.tar.gz` (Node SEA, Node 26.8.2) on `pingdotgg/t3code` Releases with `SHA256SUMS` (unsigned); same version/commit as the desktop build, not byte-identical; accepts the desktop envelope (`--bootstrap-fd 0`); native addons N-API; macOS arm64 only; Developer ID or ad-hoc signed | `scripts/build-cli-archive.ts`; `.github/workflows/release.yml:690-820`; `docs/operations/release.md:40-43` | missing | exact2 asset limits still apply (X4) | size; nightly retention; redistribution license review |
| E2a Local environment switch | Restart dialogs; off-state home "Connect to a computer running T3 Code" | `LocalEnvironmentSetting.tsx` | missing | window opens after backend ready | relaunch vs restart in place |
| E2b Network access / Tailscale | dialogs, endpoint rows, Setup/Disable, port 1–65535 | `ConnectionsSettings.tsx:2968-3825`; `DesktopServerExposure.ts` | missing | no-network typed error | Tailscale CLI location |
| E2c Authorized clients | create link (8 scopes, presets), QR, revoke | `ConnectionsSettings.tsx:221-1237` | missing | hosted pairing URL | keep or drop hosted link |
| E3 `t3 app <dir>` | Unix socket broker; eight error codes | `DesktopAppActivation*.ts`; `server/src/cli/app.ts` | missing | shared `~/.t3` → the socket path collides with T3 Code (Nightly) (accepted: one app at a time) | `t3` on PATH |
| E4 `t3code://` | hosted-web → desktop Codex handoff, registered by the **Clerk** bridge | `apps/desktop/src/app/DesktopClerk.ts:127-193` | missing | Codex loopback is IPC, not a scheme | belongs to the excluded Clerk feature? (open decision) |
| E5 misc | SSH password prompt (askpass); favicon picker; theme file picker (256 KiB, `~/.vscode/extensions`); system locale; `probeRemoteEditors`; `RunningThreadKeepAlive` is a subscription policy (not sleep prevention); Full Disk Access is Browser-only (excluded) | `packages/ssh/src/auth.ts`; `ipc/methods/window.ts` | partial / missing | — | — |
| E6 menus / quit | ⌘W repeat block; conceal by opacity 0 + leave full screen; async mode read | `DesktopWindow.ts:634-662`; `QuitHold.ts` | menus and hold done; deltas missing | Develop/Go hidden | — |
| DesktopBridge checklist | 30 member groups | `packages/contracts/src/ipc.ts:1124-1259` | done: badge, SSH, SnapShot, pickers, menus, quit hint; missing: local env, exposure, password prompt, favicon, provider auth callback, fullscreen state, activation, locale, remote editors; excluded: WSL, update, preview | — | — |
| Scopes | clone requests 3; reference standard is 5 incl. `terminal:operate`; local desktop gets all 8 | `T3Transport.swift:310,365` | partial | — | — |

### Terminal, floating player, font size

| Workflow | Observed behavior | Source | Clone status | Edge cases | Uncertainty |
| --- | --- | --- | --- | --- | --- |
| B1 drawer | 280 default, 180 min, 75 % max; per-thread state `t3code:terminal-state:v1` v4 | `ThreadTerminalDrawer.tsx:93-105,1309-1377` | missing (toggle disabled) | — | resize cursor |
| B1 tabs/splits | 144 px list with 2+; 4 per group; groups unlimited | `terminalUiStateStore.ts:254-348` | missing | split at limit does nothing | — |
| B1 panel surface, close/exit | `terminal:<id>`; destructive confirm; `[terminal] Process exited` then close | `rightPanelStore.ts`; `terminalCloseConfirm.ts` | partial / missing | "Close others" skips confirm | — |
| B1 session/RPC | open, attach, write, resize, close, metadata stream; 512 KiB client buffer; server pauses after 8 chunks / 64 KiB without Ack | `client-runtime/.../terminal.ts`; `OutputProtocol.ts` | missing | up to 44 attach streams vs the 16-stream cap | — |
| B1 input/links/clipboard/chat/scripts/sidebar/keys/settings | ⌘C/⌘V, ⌥/⌘ arrows, ⌘K, links by setting, Add to chat chips, Run in terminal, "Open terminal", "N terminal processes running", ⌘J/D/⇧D/N/W | `surface.ts`; `terminalContext.ts`; `ChatMarkdown.tsx:1003-1133` | missing | 65,536-char write cap | clipboard read inside WKWebView |
| Terminal assets | MIT JS source + committed WASM (`ghostty-vt.wasm` 630,932 B; `ghostty-write-pty.wasm` 112 B) + Nerd Font (1,177,576 B); Ghostty pin `9f62873b…` | `apps/web/src/terminal/ghostty/` | missing | Zig download unchecked: do not rebuild in the normal build | Nerd Fonts release |
| D2 / TN1 sign-in terminals | ACP terminal auth via `provider.auth.respond`; onboarding Install/Sign in terminal | `ProviderAuthTerminal.tsx`; `WelcomeWizard.tsx:960-1130` | missing | 4,096-char input chunks | PTY in the embedded runtime |
| B3 floating player | drag pill, 8 grab zones, aspect-locked, 240×150 min, avoid composer/cards, float on panel close | `previewMiniPlayerLayout.ts`; `chatCanvasLayout.ts` | partial (float/close/restore done; drag/resize missing) | lane limits in rem (B3↔B5) | grab zones outside the frame |
| B5 interface font size | 12–20 px root font size; prompt/code/diff/terminal stay px | `appearanceFonts.ts:95-123` | partial (stored; only sidebar min width reads it) | 1,622 `font-size=` sites | framework root size (X3) |

### New gaps found by the investigation (IDs used by tickets)

| ID | Gap | Source | Ticket |
| --- | --- | --- | --- |
| CN1 | A server at or after `5a96895a85` withholds Read and skill output; those rows must fetch it | `itemDetail.ts`; `WireProjection.ts` | 20261005-upstream-timeline-and-markdown |
| CN2 | Offline banner: 2 s grace during updates and a "Disconnect server" action | `ChatView.tsx:2590-2600` | 20261005-server-update-banner |
| CN3 | "Client and server versions differ" card | `ThreadDetailsPanel.tsx:131-150` | 20261005-server-update-banner |
| CN4 | Usage page covers one environment | `CR/state/usage.ts:85-125` | 20261005-usage-pooled-view |
| CN5 | `LegacyThreadMigrationToast` ("Restoring your threads…") | `LegacyThreadMigrationToast.tsx` | 20261005-local-primary-environment |
| TN1 | Onboarding "Install" / "Sign in" opens an inline terminal | `WelcomeWizard.tsx:960-1130` | 20261005-sign-in-terminals |
| TN2 | Terminal RPCs need scope `terminal:operate` | `RpcAuthorization.ts:173-181` | 20261005-remote-scopes-and-update-commands |
| TN3 | Thread delete closes its terminals and clears terminal UI state | `useThreadActions.ts:494-526` | 20261005-terminal-drawer |
| TN4 | Port `projectScriptRuntimeEnv` / `projectScriptCwd` | `packages/shared/src/projectScripts.ts:49-58` | 20261005-terminal-drawer |
| TN5 | Chat lane limits are in `rem`, so the floating player and font size interact | `ChatCanvas.tsx:128-131` | 20261005-floating-device-player, 20261005-interface-font-size |
| TN6 | Mini player position is not persisted (reference behavior to keep) | `previewMiniPlayerStore.ts:64` | 20261005-floating-device-player |
| TN7 | Right-panel tab context menu (Rename, Copy path, Close, Close others, Close to the right, Close all) | `RightPanelTabs.tsx:905-990` | 20261005-right-panel-tab-menu |
| TN8 | Device surfaces are keyed by host and device in the reference; the clone keeps one device surface per thread | `rightPanelStore.ts`; `rightPanelStore.test.ts:30` | 20261005-right-panel-tab-menu |

Line numbers in this plan come from the mc-orch tree on 2026-10-05 (clone) and from
`1e2ecbd975` (reference). `20261005-hot-file-split` moves clone code, so implementers find
code by symbol; line numbers are hints.

### Done items found by the investigation (no ticket)

R12 T2 "Worktree ready" card; Monospace font row (draft A18 was wrong); list skeleton for PRs
(C7 part); Merge dialog on the thread card (C14 part); ⇧⌘V and Speech menu items; Dock badge;
quit hold; SSH environment operations; menu bar. Description reactions are not rendered by
the reference either (C6 part: do not build).

### Corrections to the draft inventory

`t3.server.origin` lives in `T3Transport.swift`; A8, A9, A11, A15 are partial; A14 has no
default chord; A16/A17 are unknown; G13's word highlight is a fixed `"none"`, not a toggle;
G14 ⇧⌘[ ] must not wrap; "5-minute TTL" and "write FIFO" for terminals have no reference
source; "Settings: cursor" does not exist; `RunningThreadKeepAlive` is not sleep prevention;
E1 removes only 10 named `T3CODE_*` variables; E4's loopback is IPC; Full Disk Access is
Browser-only.

## Library-based feasibility

Investigated by a separate agent with the bundled library only (revision
`20261005-platforms-v3`; topics capabilities, foundations, components, state-and-data,
layout-and-interaction, design, motion, accessibility reviewed at `fd1c879`; platforms,
performance, testing-and-debugging at `e45434f`). Every topic is source-reviewed and not
runtime-tested, so "supported" means source-documented.

| Requirement | Bundled topic and revision | Support by platform (macOS) | Limitation or knowledge gap | Planning consequence |
| --- | --- | --- | --- | --- |
| Routes, tab stacks, modals | capabilities, layout-and-interaction @fd1c879 | supported ("documented for web, macOS, iOS") | route boxes do not scroll | bounded scroller named by `navigationScroll`; test back, tab return, dismissal |
| Settings pages, sidebar selection, tabs | layout-and-interaction, design @fd1c879 | supported as a pattern (native list sections, semantic tabs) | native faces have restricted styling | prove selection and styling against the oracle |
| Dialogs, popovers, tooltips | capabilities, accessibility @fd1c879 | dialogs partly ("modal navigation"); popovers/tooltips **unknown** | dialog focus trapping/restoration not established | knowledge gap 6; clone runtime evidence is the current basis |
| Long virtualized lists | capabilities, layout-and-interaction, performance | supported with compiler-enforced shape restrictions | variable row height, anchoring, prepend not stated; horizontal virtualization needs literal flex + positive literal height, no wrap/gap/reorder | `perf` workload per list ticket; knowledge gap 11 |
| Resizable panes, drag handles | motion, capabilities | **unknown** ("pointer actions" named, not established) | — | knowledge gap 7; held-gesture rule: keep the pressed node alive |
| Resources and mutations | state-and-data, foundations | supported (root resources, mutations, `refresh`, `pending`, `failed`) | never send the same mutation twice on one action path | test success, pending, transport failure, domain failure, late reply |
| Streaming subscriptions, optimistic updates | state-and-data | **unknown** | — | knowledge gap 1 |
| Persisted preferences | state-and-data, design | supported with rules (await writes on macOS; restore must call `setScheme`) | serialize SQLite work | `reload` with a named store; relaunch from a saved value |
| Keychain credentials | state-and-data | **unknown** (only "keep credentials below the resource identity seam") | — | knowledge gap 2 |
| Two-way WebSocket RPC; data-module timers | foundations, components, testing | **unknown** (`clock +N` exists for testing timers) | — | knowledge gap 1; issues X19, X21 |
| Embedded native views (terminal WKWebView, 3D, PDF, video) and their agent driving | capabilities, testing | **unknown** ("arbitrary native integrations: unknown in this library") | real media is not virtual-clock playback | knowledge gap 3; attended sessions; issue X8 |
| Child process, bundled runtime, Unix socket, loopback HTTP, URL scheme, save panel, pasteboard, menus, title bar, quit hook, notifications, Dock badge | capabilities, platforms | **unknown** (platforms lists menu/window lifecycle only as test priorities) | — | knowledge gap 4; issues X4, X5, X6, X26–X28 |
| Window default/minimum size | platforms, capabilities | agent resize supported on macOS; size control **unknown** | — | knowledge gap 5 |
| Light/dark/system | design @fd1c879 | supported (`setScheme`, `light-dark()`) | do not branch colors on `prefersColorScheme` | test with `prefer prefers-color-scheme` |
| Root font size (rem 12–20 px) | foundations, design | **unknown** | — | knowledge gap 10; issue X3 |
| Drawer/panel transitions | motion, capabilities | property transitions, keyframes, springs supported; **general layout interpolation unsupported** | `exit-animation`, `layout-transition` not proven | use transforms or a proven measured layout-transition |
| Hover cards with delay | design, capabilities | **unknown** (cursor hints supported on macOS) | cursor alone must not convey an interaction | knowledge gap 8; issue X13 |
| Press feedback, reduced motion | motion, accessibility | supported (`press-scale`, `exactViewport().prefersReducedMotion`) | the app authors the policy | reduced-motion branch per effect |
| ARIA labels and states | accessibility | supported (`aria-label`, `-expanded`, `-pressed`, `-live`, `-hidden`, `inert`, `autofocus`) | tree is not proof of assistive-technology behavior | `tree --ax`; manual VoiceOver steps where named |
| Shortcuts with modifiers, key-up, IME | accessibility | **unknown** | — | knowledge gap 9; issues X15, X25; attended IME checks |
| Agent drive and resize on macOS | platforms, testing, capabilities | supported (`agent macos`; resize supported) | build must precede driving | both window sizes per ticket |
| Real-input limits | testing | partly (a layout box is not proof of a tappable center) | OS-level input/IME not described | attended real-input checklist; knowledge gap 13 |
| Long-list performance | performance @e45434f | supported (`perf … during`, `perf frames`) | no thresholds; missing field is not zero | each list ticket sets a budget and a baseline |
| Downloadable `.app` | platforms | **unknown** (only `mac --run`) | — | knowledge gap 12; ticket `20261005-portable-app-download` |

### Knowledge-update handoff (`update-best-practices`)

1. Data transport: two-way WebSocket, streaming subscriptions, optimistic updates, data-module timers (state-and-data, components).
2. Keychain credentials (state-and-data).
3. Embedded native views and their agent driving (new topic + testing).
4. macOS host integration: child process, bundled runtime, sockets, loopback HTTP, URL scheme, panels, pasteboard, menus, title bar, quit hook, notifications, Dock badge (new topic + platforms).
5. Window default and minimum size (platforms).
6. Popovers, tooltips, dialog focus trap and restore (layout-and-interaction, accessibility).
7. Resizable panes and pointer drag (layout-and-interaction, motion).
8. Hover events and delays (motion, layout-and-interaction).
9. Keyboard shortcuts, modifier/key-up state, IME composition (new input topic).
10. Root font size and `rem` (design, foundations).
11. Variable-height virtualized rows, scroll anchoring, prepend (layout-and-interaction, performance).
12. Packaging an `.app` that runs on another Mac (new distribution topic).
13. macOS real-input path (testing).

The clone already uses most of these at runtime on exact2 `c1522fdac` (round-11 evidence:
26 AppKit test binaries, agent drives, pixel matrix). That is application evidence, not
library knowledge. Spec open decision 1 chooses between the handoff and accepting the
clone's runtime evidence on the pinned `main` ticket by ticket.

### Documented limitations that affect tickets

- Horizontal virtualization: literal flex layout and positive literal height; no wrap,
  reverse, gap, main-axis padding or reorder (terminal tab strips, right-panel tab strip,
  chip rows must not rely on them).
- No general width/layout interpolation: drawer and pane size changes animate with
  transforms or not at all.
- Route boxes do not scroll; boxes are content-box; flex children may need `min-width=0`
  / `min-height=0` — check every pane at 840×620.
- Actions read a snapshot; resources, mutations and scheduled tasks are root-owned
  (debounce, retry and optimistic logic stay at the root).
- Await persistence on macOS; bake-time storage is unavailable (catch only `code === 'bake'`).
- Keep a pressed node alive through a held gesture (drag handles, sweep).

## Research review

Conflicts found and their resolution:

1. **Embedded server source.** The draft design packed `apps/server/dist` + `node_modules`
   + official Node from the reference checkout. The user then required (2026-10-05) the
   original's own server and a build without the reference checkout. The desktop app runs
   the server with Electron as Node (not reusable without shipping Electron); the official
   plain-Node form is the CLI archive (Node SEA) from GitHub Releases with `SHA256SUMS`, same
   version and commit as the desktop build. Resolution: recommend the CLI archive; record it
   as an open decision because it is not byte-identical to the desktop bundle.
2. **Shared `~/.t3`** (user decision) collides with T3 Code (Nightly) on the activation socket,
   port 3773 and the database. Resolution: accepted consequence; one app at a time in
   production; lanes isolate.
3. **`lower-alertdialog`.** The draft rebase step expected 9 compile fixes; `EXACT2-GAPS.md`
   says main does not refuse those columns. Resolution: the clone-on-main ticket fixes them
   only if main refuses.
4. **E4 `t3code://`.** Its handler belongs to the Clerk bridge, which the spec excludes.
   Resolution: open decision in `20261005-app-activation`.
5. **Local scopes.** The clone requests 3 scopes; the reference desktop gets all 8 for the
   local session and 5 for remote pairing. Resolution: fixed in
   `20261005-local-primary-environment` and `20261005-environment-routes`.
6. **"Local only"** in the user's message meant "no deployment", not fewer features
   (answer 2026-10-05). Resolution: full scope; new ticket `20261005-portable-app-download`.

**Boundary breach (disclosed).** While writing the terminal tickets, one investigator ran
`ls scripts` and a grep of `scripts/agent.mjs` in an exact2 checkout, which the planning
rules forbid. It used the result only to confirm that an agent pointer-down/up form exists;
the tickets cite the clone's own `AGENT-HANDOFF.md` for that fact instead. No other framework
file was opened.

Unresolved questions are listed in spec "Open decisions" and in each ticket. Apart from the
breach above, no framework source was read in this planning stage; framework facts come from the bundled library or are
quoted from the clone's `EXACT2-GAPS.md` with that label.
