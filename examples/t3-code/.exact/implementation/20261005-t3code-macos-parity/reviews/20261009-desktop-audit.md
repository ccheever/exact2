# Desktop audit, 2026-10-09

This audit compares the running Electron desktop app with the running Exact clone, area by area. It adds tracking
records only. It does not fix anything and does not accept existing tasks.

Result:
- 81 findings in seven areas.
- 68 findings go to 15 new tasks.
- 3 findings go to 2 blocked tasks.
- 1 new local framework draft: X69.
- 10 findings are already tracked by existing records.
- 3 root causes were found in more than one area. Each is one item in one record.
- 2 findings need the user's decision.

## Baseline and method

- **Reference.** T3 Code pin `1e2ecbd975` ("T3 Code (Alpha)", 0.0.45). It ran as a real Electron production build from
  the source copy `target/t3-ref/src-1e2ecbd975`, and its renderer was driven over CDP (synthetic events in the page).
- **Clone.** `examples/t3-code` at `feat(example)/t3-code` `c603c22d6`, a development build. The agent driver
  (`scripts/agent.mjs macos`) drove it in a 1280×840 window, with one `--storage audit-<lane>` per lane.
- **Lanes and tools.** Everything is under `target/t3-audit`:
  - `ref-app.sh`, `ref-stop.sh` and `ref-cdp.mjs` start, stop and drive the reference;
  - `clone-drive.sh` runs one clone session;
  - `lanes/<lane>/` holds each lane's homes, its `work` project and its helper scripts.
  Each area had its own lane and ports (16800–16902). Both apps started with the same data: the "Verification fixture"
  project and a private git project `work` (branch `feature/audit`).
- **No real input.** The screen was locked. No `orca computer`, no `screencapture`. The reference's native menus and
  dialogs, real drags and OS flows were compared from source and the clone's agent tree. Those rows are marked
  `needs_real_input`.
- **Seven areas, one auditor each.** shell, thread, composer, panel, settings-1, settings-2 and pages. Clone sessions per
  area: shell about 30, thread 11, composer 16, panel 19, settings-1 25, settings-2 8, pages 13. One reference instance per
  lane; every one was stopped at the end.
- **Existing records were checked first:**
  - `tasks/` and `tasks/closed/` (rows marked not done or residual count);
  - `issues/` and `issues/README.md`;
  - the [2026-10-07 review](20261007-desktop-clickthrough.md);
  - `examples/t3-code/EXACT2-GAPS.md` and `examples/t3-code/STATUS.md`;
  - the open Browser PRs.
- **Charlie's decisions are binding.** On 2026-10-09 he moved the open GitHub issues to filesystem issues on `main`. This
  review cites them as `main` files (read at `origin/main` `a468874f4`).
- **This review** checked each finding for plausibility against the cited sources, merged findings with one root cause,
  and checked each `known_record` against its record.

## Fixture limits and side effects

- **Unequal providers.** `clone-drive.sh` did not isolate HOME and PATH for the clone's embedded server.
  - The clone found `~/.local/bin/claude` 2.1.295 (signed in) and Codex 0.160.1.
  - The reference, under `env -i` with the lane HOME, found no Claude and Codex 0.151.0 ("Unsupported").
  - The composer auditor set the provider binary paths to compare equal states.
  - Settings-1 did not compare the provider list or the model rows.
  - Isolate the clone lane's HOME and PATH before provider work.
- **Server versions.** The reference server is the 0.0.45 pin; the clone's runtime is 0.0.46 nightly. Version strings,
  the Nightly toast and the Nightly-only rows were excluded.
- **Agent clock.** It starts at 2026-01-01 UTC. Later drives used `--epoch <now> --time-zone Asia/Seoul`.
- **Agent storage.** Module data does not follow `--storage` (X50, #284), so persistence across a relaunch was not
  compared. ⌘Z and ⌘W through `R8KeysMenus` need real keys.
- **Diff data.** The reference server refused the `work` cwd ("configured workspace root") and showed its own checkout.
  Diff controls were compared; diff data was not.
- **Side effects:**
  - Shell lane: the palette's Git URL clone resolved `~/work` to `/Users/daehyeonmun/work`. The auditor checked that it
    came from the lane's `file://` repo and removed it.
  - Shell fresh lane: an inexact "Continue" click in the reference hit "Continue with ChatGPT" and started a managed Codex
    download. It was cancelled within seconds; no files remained.
  - Panel lane, clone drive 5: a tap meant for the git menu hit the primary Commit. "Generating commit message..." failed
    because Codex is not authenticated. No commit was made, and the lane's git log is unchanged.
  - Settings-2: the reference's thread was archived through its page's own WebSocket (`thread.archive`), then unarchived
    in the page.
  - No message was sent to a provider.
- **Lane state left behind:**
  - Keybindings files are restored.
  - The shell lanes have an extra cloned project `work`.
  - The panel lane homes have "Audit work thread" (its bearer sessions are revoked) and a `work-wt` worktree.
  - The thread lane homes have the extras threads (backups `statev2.sqlite.pre-extras`).
  - The composer reference lane has provider binary paths set.
  - The pages lanes have synthetic transcripts and one custom price.

## Coverage

Each auditor first listed the area's items from the reference source, then compared each item. A "diff" row can hold
several findings, and a finding can span rows: 76 diff rows hold the 81 findings.

| Area | Items | Match | Diff | Not compared | Why not compared |
| --- | --- | --- | --- | --- | --- |
| shell | 50 | 33 | 6 | 11 | real input (About panel, drag and drop, sidebar resize, ⌘Z, ⌘W, the quit hold); a running provider turn (Working, Approval shelves); ⌘O launches an editor; closed scope (desktop update toast, `/connect`); not reachable (no-active-thread state) |
| thread | 51 | 34 | 9 | 8 | the server does not surface seeded pending approval and question requests; no attachments, checkpoints, error or woke banner in the fixture; archive is destructive; long output and clipboard results not captured |
| composer | 34 | 22 | 8 | 4 | native file drop and `NSOpenPanel` (real input); queue, steer and stop need a running turn; no usage data; the fixture thread is too short for the resting composer |
| panel | 46 | 25 | 13 | 8 | the primary Commit calls a provider; no origin remote; the composer strip is another area; file editing was covered on 2026-10-07; diff data is environmental; the terminal context menu is native; Browser part 5 Mute and automation; no PR fixture |
| settings-1 | 59 | 32 | 18 | 9 | unequal provider fixture (Model, Text generation model, provider list); Nightly-only rows; Export theme (OS save); Open keybindings.json and Save (external editor, file write); no `t3.json`; SnapShots step 2 (window capture); installs (ACP Add, Update all, enable switches) |
| settings-2 | 53 | 37 | 11 | 5 | Browser rows (open PRs #354, #353 at the time); no task saved (to avoid provider runs); Tailscale not running; no saved remote environment; the archived row's native menu and Delete |
| pages | 38 | 16 | 11 | 11 | no Cursor or ChatGPT account; per-process agent data (restored prefs, toast dismissal); no GitHub remote; SnapShot capture (OS shortcut, grants); Dock badge (#224); launches other apps; normal relaunch (window bounds, single instance); closed scope (deep links); rows owned by other areas |
| **Total** | **331** | **199** | **76** | **56** | |

Matching surfaces have screenshots in `target/t3-audit/evidence/<area>/match/`.

## Findings and records

New tasks are in `../tasks/`. "Existing" means the record already covers the finding, and no new record was made.

| Id | Finding | Record |
| --- | --- | --- |
| SH-1 | ⌘N and ⇧⌘N do nothing while a sidebar row has the focus | [shell-sidebar-palette-keys](../tasks/closed/20261009-shell-sidebar-palette-keys.md) |
| SH-2 | ⇧⌘S settles with no undo notice and no undo step | [shell-sidebar-palette-keys](../tasks/closed/20261009-shell-sidebar-palette-keys.md) |
| SH-3 | "New thread in…" rows have no ⌘1–⌘9 hints or picks | [shell-sidebar-palette-keys](../tasks/closed/20261009-shell-sidebar-palette-keys.md) |
| SH-4 | No-projects header shows Filter and Add project | [shell-sidebar-palette-keys](../tasks/closed/20261009-shell-sidebar-palette-keys.md) |
| SH-5 | Palette row path is clipped at its start | [shell-sidebar-palette-keys](../tasks/closed/20261009-shell-sidebar-palette-keys.md) (root cause X57, #291; clone `text-align="left"`) |
| SH-6 | No "Check for updates" in the sidebar footer | blocked: [blocked-desktop-update-controls](../tasks/closed/20261009-blocked-desktop-update-controls.md) |
| SH-7 | Blue focus ring on the palette field | existing: [X61](../issues/closed/20261008-x61-field-focus-ring-opt-out.md) (#302, waits for main fix) |
| SH-8 | View › Toggle Developer Tools is absent | existing: [X2](../issues/closed/20261005-x02-app-developer-tools.md) (permanent declared difference; `rules/DEFERRED.md:722`) |
| TH-1 | Fork shows on a message with no run and only errors | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| TH-2 | Notification source kinds are not decoded | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| TH-3 | Work group icon ignores the tools' icons | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| TH-4 | Expanded group stays capped at 18rem | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| TH-5 | Search and file-change rows lack the inspector details | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| TH-6 | Subagent card icon and elapsed format | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| TH-7 | Expanded Mermaid preview on a black box | [app-color-scheme](../tasks/closed/20261009-app-color-scheme.md) (one root cause with PG-1, PA-11) |
| TH-8 | Custom snooze date is a stepper, not a calendar button | [shell-sidebar-palette-keys](../tasks/closed/20261009-shell-sidebar-palette-keys.md) |
| TH-9 | Relative `:line` links render as extra, dead chips | [markdown-links-and-files-preview](../tasks/closed/20261009-markdown-links-and-files-preview.md) (decision needed) |
| TH-10 | Tool image icons not muted | [timeline-work-rows](../tasks/closed/20261009-timeline-work-rows.md) |
| CO-1 | Model rows have no ⌘1–⌘9 badge | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| CO-2 | Unavailable rail button: "Not ready." and no tooltip | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| CO-3 | Search does not highlight the first match; Enter does nothing | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| CO-4 | ⇧⌘↓ moves to an unavailable provider | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| CO-5 | "Add project script" opens Settings, not the dialog | [composer-provider-state-and-details](../tasks/closed/20261009-composer-provider-state-and-details.md) |
| CO-6 | Missing catalog model shows the stored slug | [composer-provider-state-and-details](../tasks/closed/20261009-composer-provider-state-and-details.md) |
| CO-7 | Shift+click closes the picker | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| CO-8 | Details card keeps "work" under multi-model | [composer-provider-state-and-details](../tasks/closed/20261009-composer-provider-state-and-details.md) |
| CO-9 | Placeholder stays "Ask for changes…" with no provider | [composer-provider-state-and-details](../tasks/closed/20261009-composer-provider-state-and-details.md) |
| CO-10 | Thread with a failing provider loses its picker | [composer-provider-state-and-details](../tasks/closed/20261009-composer-provider-state-and-details.md) |
| CO-11 | Picker name and Plan pressed state differ | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| CO-12 | Focus ring on the composer and search fields | existing: [X61](../issues/closed/20261008-x61-field-focus-ring-opt-out.md) (#302) |
| PA-1 | Device disabled on a draft | [right-panel-launcher-and-files](../tasks/closed/20261009-right-panel-launcher-and-files.md) |
| PA-2 | Launcher has no arrow keys or Enter | [right-panel-launcher-and-files](../tasks/closed/20261009-right-panel-launcher-and-files.md) |
| PA-3 | Files rendered Markdown uses a reduced renderer | [markdown-links-and-files-preview](../tasks/closed/20261009-markdown-links-and-files-preview.md) |
| PA-4 | No "Open file in preview browser" | [right-panel-launcher-and-files](../tasks/closed/20261009-right-panel-launcher-and-files.md) |
| PA-5 | Image preview shows a word-wrap toggle | [right-panel-launcher-and-files](../tasks/closed/20261009-right-panel-launcher-and-files.md) |
| PA-6 | No base-ref label or comparison picker | [diff-panel-parity](../tasks/closed/20261009-diff-panel-parity.md) |
| PA-7 | Scope menu hides Latest turn and Turn | [diff-panel-parity](../tasks/closed/20261009-diff-panel-parity.md) |
| PA-8 | File header stats show deletions first | [diff-panel-parity](../tasks/closed/20261009-diff-panel-parity.md) |
| PA-9 | No Workspace select on an unstarted server thread | [composer-provider-state-and-details](../tasks/closed/20261009-composer-provider-state-and-details.md) |
| PA-10 | Terminal tab has no icon | [right-panel-launcher-and-files](../tasks/closed/20261009-right-panel-launcher-and-files.md) |
| PA-11 | Terminal dark while the app is light | [app-color-scheme](../tasks/closed/20261009-app-color-scheme.md) |
| PA-12 | ⌘Enter does not save a diff comment | [diff-panel-parity](../tasks/closed/20261009-diff-panel-parity.md) (regression) |
| PA-13 | Launcher Browser row has no profile chevron | existing: [browser-surface-profiles](../tasks/closed/20261005-browser-surface-profiles.md) (#354) |
| PA-14 | Browser parts 2–4 rows missing or disabled | existing: [browser-surface-navigation](../tasks/closed/20261005-browser-surface-navigation.md) (#352), [browser-surface-capture](../tasks/20261005-browser-surface-capture.md) (#349), [browser-surface-profiles](../tasks/closed/20261005-browser-surface-profiles.md) (#354); Open DevTools is declared (`EXACT2-GAPS.md:430`) |
| S1-1 | Escape in a field's popup closes Settings | [settings-escape-and-nav](../tasks/closed/20261009-settings-escape-and-nav.md) (one root cause with S1-2, S2-1) |
| S1-2 | Escape in the custom model field closes Settings | [settings-escape-and-nav](../tasks/closed/20261009-settings-escape-and-nav.md) (regression) |
| S1-3 | Escape while recording closes Settings | [settings-escape-and-nav](../tasks/closed/20261009-settings-escape-and-nav.md) |
| S1-4 | Recording a bound chord runs its command | [settings-escape-and-nav](../tasks/closed/20261009-settings-escape-and-nav.md) |
| S1-5 | Duplicate and Edit open the editor in simple mode | [settings-appearance-and-skill-chip](../tasks/closed/20261009-settings-appearance-and-skill-chip.md) |
| S1-6 | Open VSX results show the login, not the namespace | [settings-appearance-and-skill-chip](../tasks/closed/20261009-settings-appearance-and-skill-chip.md) |
| S1-7 | No colour usage highlight or "N uses" | blocked: [blocked-theme-usage-highlight](../tasks/20261009-blocked-theme-usage-highlight.md); new issue [X69](../issues/closed/20261009-x69-paint-role-node-query.md) |
| S1-8 | Restore defaults button reads "Restore defaults" | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S1-9 | Background activity popover shows "Balanced" | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S1-10 | Colour picker covers the editor header | existing: [X17](../issues/closed/20261005-x17-popover-position-try.md) (#112; no per-site flip arithmetic) |
| S1-11 | Font pickers offer a fixed catalog | existing: [installed-font-picker](../tasks/20261007-installed-font-picker.md) (X48, #318) |
| S1-12 | Skill chip opens no details popover | [settings-appearance-and-skill-chip](../tasks/closed/20261009-settings-appearance-and-skill-chip.md) |
| S1-13 | Update track disabled; no nav "Check for updates" | blocked: [blocked-desktop-update-controls](../tasks/closed/20261009-blocked-desktop-update-controls.md) |
| S1-14 | Days of inactivity is a text field | existing: [X60](../issues/closed/20261008-x60-number-field-semantics.md) (#301) |
| S1-15 | Theme editor has no Inspect | existing: [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md) line 137 (U18, waits for X68, #321) |
| S1-16 | No hover tooltip on Background policy details | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-1 | Escape in a Settings dialog also leaves Settings | [settings-escape-and-nav](../tasks/closed/20261009-settings-escape-and-nav.md) |
| S2-2 | Diagnostics stays empty (request forgotten) | [settings-diagnostics-and-scope](../tasks/closed/20261009-settings-diagnostics-and-scope.md) (high; regression) |
| S2-3 | Checkout-path toast title and description | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-4 | New task and writer model use a flat list | [model-picker-parity](../tasks/closed/20261009-model-picker-parity.md) |
| S2-5 | Settings sidebar has no resize rail | [settings-escape-and-nav](../tasks/closed/20261009-settings-escape-and-nav.md) |
| S2-6 | Base branch says "From origin/main" for an unknown ref | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-7 | Git details row title and missing info button | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-8 | Invalid host error text | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-9 | Icon submenu opens below, rows centred | existing: [X17](../issues/closed/20261005-x17-popover-position-try.md) for the placement (Charlie, line 13); the row alignment is in [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-10 | Accessible names and roles differ | [settings-rows-and-labels](../tasks/closed/20261009-settings-rows-and-labels.md) |
| S2-11 | Diagnostics switches the scope to one environment | [settings-diagnostics-and-scope](../tasks/closed/20261009-settings-diagnostics-and-scope.md) |
| PG-1 | Usage colours follow macOS, not the app | [app-color-scheme](../tasks/closed/20261009-app-color-scheme.md) |
| PG-2 | USD half cents round down | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) |
| PG-3 | Unpriced popover does not open on hover | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) |
| PG-4 | Toggles have no shortcut tooltips | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) |
| PG-5 | Environment name truncated in its menu | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) |
| PG-6 | Author submenu has no search field | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) |
| PG-7 | Escape does not leave Pull Requests | [usage-and-pr-pages](../tasks/closed/20261009-usage-and-pr-pages.md) |
| PG-8 | Project page opens scoped to one environment | [settings-diagnostics-and-scope](../tasks/closed/20261009-settings-diagnostics-and-scope.md) |
| PG-9 | No permission helper beside System Settings | [snapshot-permission-helper](../tasks/closed/20261009-snapshot-permission-helper.md) |
| PG-10 | Notifications watch only the focused environment | [notifications-all-environments](../tasks/closed/20261009-notifications-all-environments.md) |

## Deduplication

- **One root cause, three areas.** PG-1 (pages), PA-11 (panel) and TH-7 (thread): views pick their palette from
  `viewport.prefersColorScheme`, the macOS appearance, not from the app's mode. One task: app-color-scheme.
- **One root cause, two areas.** S1-1, S1-2 (settings-1) and S2-1 (settings-2): Settings' Back keeps its Escape shortcut
  while a field, editor or dialog inside owns Escape. One task: settings-escape-and-nav.
- **One existing issue, two areas.** SH-7 and CO-12 are X61. S1-10 and S2-9's placement are X17.
- **One shared control.** S1-16 and S2-7 need the same "Background policy details" info button.

## Already tracked

- X61 ([#302](https://github.com/ccheever/exact2/issues/302); `main` `issues/20261009-field-outline-none.md`): SH-7, CO-12.
  Approved; waits for main fix.
- X2 ([#101](https://github.com/ccheever/exact2/issues/101)): SH-8. Permanent declared difference (X2 line 13;
  `rules/DEFERRED.md:722` "no devtools UI").
- X17 ([#112](https://github.com/ccheever/exact2/issues/112); `main` `issues/20261009-popover-css-flip-fallbacks.md`): S1-10
  and S2-9's placement. Charlie: T3 waits and adds no per-site flip arithmetic (X17 line 13).
- [installed-font-picker](../tasks/20261007-installed-font-picker.md) (X48, [#318](https://github.com/ccheever/exact2/issues/318);
  `main` `issues/20261009-runtime-installed-font-family.md`: a decision is pending under LLP 1019/1053): S1-11.
- X60 ([#301](https://github.com/ccheever/exact2/issues/301); `main` `issues/20261009-macos-number-field-behavior.md`): S1-14.
- [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md) line
  137 (U18; X68, [#321](https://github.com/ccheever/exact2/issues/321)): S1-15.
- Browser part tasks (open PRs #352, #349, #354): PA-13, PA-14.

## Regressions and gaps against closed records

| Closed record | What it says | What the audit saw | Now in |
| --- | --- | --- | --- |
| [2026-10-07 review](20261007-desktop-clickthrough.md) (Diagnostics row) | Diagnostics opened with its tables | S2-2: the page stays empty | settings-diagnostics-and-scope |
| `EXACT2-GAPS.md:386` | an open custom model editor makes Back give up Escape | S1-2: Back still takes it | settings-escape-and-nav |
| [diff-review-engine](../tasks/closed/20261005-diff-review-engine.md) lines 101, 133 | ⌘↵ saves a line comment | PA-12: ⌘Enter leaves the draft open (agent keys; real keys to check) | diff-panel-parity |
| [upstream-timeline-and-markdown](../tasks/closed/20261005-upstream-timeline-and-markdown.md) A4, lines 38, 106 | relative `name:line` links: prose + chip, chip only | TH-9: the live reference strips those hrefs | markdown-links-and-files-preview (decision) |
| [app-update-feed](../tasks/closed/20261005-app-update-feed.md) lines 19, 91 | the disabled updater equals the reference; "no pill" | SH-6, S1-13: the reference with no feed draws three controls | blocked-desktop-update-controls (decision) |

Seen before but never tracked:
- TH-8: [desktop-shell-details](../tasks/closed/20261005-desktop-shell-details.md) line 117 notes the date input.
- S1-12: [skill-chip-provider-name](../tasks/closed/20261008-skill-chip-provider-name.md) lines 82-84, "Found, not in this task".
- S2-4: [settings-model-picker](../tasks/closed/20261007-settings-model-picker.md) covered General's pickers only.
- S1-7: [settings-scoped-controls-and-theme-editor](../tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md)
  line 31 excluded Inspect only.

## Decisions needed

- TH-9 (relative `name:line` links): settled by the user's rule of 2026-10-09 (match the reference at the pin, even where
  it looks broken): build the live reference's output. Details:
  [markdown-links-and-files-preview](../tasks/closed/20261009-markdown-links-and-files-preview.md).
- SH-6, S1-13 (the reference's no-feed update controls): the user's call. The coordinator builds option (a) as a draft
  PR; merging it builds them, closing it keeps them out under the X40 scope decision. Details:
  [blocked-desktop-update-controls](../tasks/closed/20261009-blocked-desktop-update-controls.md).

## Observations not filed

- **Agent-mode appearance.** In mode System with the Mac in Dark, the agent window drew light while
  `viewport.prefersColorScheme` read dark (pages and panel auditors).
  - `DisplayPreferences.systemDark` reads `Agent.systemAppearance` when it is set, else the Mac's `AppleInterfaceStyle`
    (`host/apple/Sources/ExactKit/DisplayPreferences.swift:91-94`).
  - `prefer` sets `Agent.systemAppearance` (`host/apple/Sources/ExactKit/Mac/AgentMac.swift:967-971`).
  - Why the window drew light was not found. No one-file repro was made, so nothing was filed.
  - [app-color-scheme](../tasks/closed/20261009-app-color-scheme.md) verifies under an explicit mode. If the mismatch still
    shows after it, the coordinator gets a one-file repro.
- **Clone composer data in agent storage.** In `--storage audit-composer` the clone never wrote `app:/data/t3-code.json`,
  so favorites, stash and drafts were gone after each relaunch. The reference keeps them. This may be X50 (#284). Confirm
  with one normal launch before filing (`target/t3-audit/evidence/composer/dumps/clone-storage-data-dir.txt`).
- **Launch toasts.** At every launch the clone shows "Update Available: Codex" and the Nightly mobile toast. They cover the
  details card and some toolbars. The Nightly toast's Dismiss does not persist in agent mode (per-process data, X50). This
  is a version difference.
- **Settings reads.** STATUS's "Found, not in scope" Integrations loop is fixed by #353 (merged as `950e8e2e5`). The other
  Settings pages' server reads are in the planned [settings-pages-subscribed-config](../tasks/closed/20261009-settings-pages-subscribed-config.md),
  which coordinates with settings-diagnostics-and-scope.
- **Driver limits:**
  - A testId that contains "/" fails as a tap target; tap by label instead.
  - A tap on a hidden `shortcut-*` button lands at the window corner.
  - `tree <id>` fails while a popover is open.
  - The reference's `ref-cdp` `goto` fails on the `t3code://app` origin; use `eval location.hash`.
  - `clickrole` matches names by substring ("Day" hit "7 days").
- **Excluded by the audit rules:** Alpha vs Nightly branding and versions; pixel differences (the user's rule).

## Evidence

Evidence stays local and is not committed: `target/t3-audit/evidence/<area>/`:
- `<id>-ref.png` and `<id>-clone.png` pairs at the same state;
- `match/` for matching surfaces;
- `dumps/`, `ref/` and `clone/` for ARIA snapshots, agent trees and drive logs.

Lane helpers are in `target/t3-audit/lanes/<lane>/tools/`. Each record restates its reproduction steps, so it does not
depend on these files.

## Validation of this tracking change

- Only `.exact` Markdown records changed:
  - 17 new task files (15 planned, 2 blocked);
  - 1 issue draft (X69);
  - this review;
  - a new section in `issues/README.md` and in `plan.md`.
- No product code changed. Nothing was committed or published.
