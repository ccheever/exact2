---
name: 20261005-settings-scoped-controls-and-theme-editor
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: pr-open
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-scoped-controls-and-theme-editor
pr_url: https://github.com/ccheever/exact2/pull/153
verified_commit: null
---

# Settings across environments, mixed switches and the app-wide theme editor

## Outcome

1. **Settings scope spans environments.** The scope menu lists every connected environment. "All environments" and a single environment read and write the real targets: a change goes to each selected connected environment, and a failure names the environments that did not save (**D15, across machines**).
2. **Mixed switches.** A scoped server-setting switch whose targets disagree (project checkouts of one environment, or several environments) draws the **mixed** state: thumb centred on a muted track, `aria-checked="mixed"`. A click turns it on for every target (**D15**).
3. **Theme editor.** The editor is a floating panel above the whole app: it opens from Settings, the command palette or its shortcut, stays open while the user moves between threads, pages and Settings, can be dragged, resized and minimized, and closes with Escape or its shortcut (**D16**).

## Scope and exclusions

Included:

1. **Scope model.** Port `validateSettingsScopeSearch`, `resolveSettingsScope` (kinds all, environment, project, checkout, unavailable with its reasons and messages), `selectScopedSettingsEnvironments`, `resolveScopedSettingsTargets`, `scopedSettingsAreMixed`, `scopedSettingsSource`, `planScopedSettingsPatch`, `planScopedSettingsClear`, `listProjectOverrides`, `planProjectOverridesClear`, `persistScopedSettingsPatch`, and the environment-label disambiguation of `settingsScopeAxis.ts`. They replace `resolveScope` (one environment, `settings-core.ts:143-166`), `settingPatch` (`:435-447`) and the local `same()` / `serverState` mixed logic (`:183-185, 211-228`). Project groups come from the focused shell and every connected fleet entry's `shell`.
2. **Data and writes.** Targets read `config.settings` and `scopes` of the focused client and of each connected `EnvironmentFleet` entry (kept live by `subscribeServerConfig`, `settings-b-fleet.ts:12-18, 138-146`). Each write goes through its own transport (`EnvironmentFleet.native(native, key)`, `connections.ts:248`, precedent `connections.ts:422-435`) and all writes are awaited together. Toasts: "Setting not saved" (warning, with the unavailable reason), "Setting saved on some environments" / "Setting not saved" (error, "Could not update `<labels>`. The other selected environments saved the change.").
3. **D15 mixed visual.** Add `mixed` to the switch row. Today `checked: state.value === true` (`settings-core.ts:267`) ignores `state.mixed` (`:216`) and the node draws on or off (`settings-rows.contract:188-190`). Rows: the seven server switches in `settings-core.ts:344-383` (`autoResumeLimitedThreads`, `snoozeLimitedThreads`, `sidebarAutoSettleOnMerge`, `sidebarAutoSettleAfterDays`, `enableProviderUpdateChecks`, `continueThreadsAfterServerUpdate`, `newWorktreesStartFromOrigin`) and the two Integrations rows (`enableDeviceSupport`, `enableAgentDeviceAccess`, `source-control-view.ts:124-127`), the same nine the reference wraps in `ScopedSwitch`.
4. **D16.** Port the session store (`openThemeEditor`, `closeThemeEditor`, `toggleThemeEditorForTheme`), host the panel at the app root instead of inside Settings, make the palette row and the `themeEditor.toggle` binding open it without opening Settings, add drag, resize and window-resize clamping, and port the save notices of `ThemeEditorHost`.

Excluded: the **Inspect** button (blocked by [X30](../issues/20261005-x30-ts-announce-readback-picker.md), plan decision U18: waive it or open a framework issue); redaction (`20261005-provider-sign-in-and-install` owns `RedactedText`); the press stretch of switches outside these nine rows; the scheduled-tasks scope filter and Usage environment filter (`20261005-live-automations-and-clones`, `20261005-usage-pooled-view`; both may use the ported `resolveSettingsScope`). Rows that are not server settings (device-local preferences) render regardless of scope, as in the reference.

## Context and guidance

Parent specification: [spec](../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `W/settings/settingsScope.ts:15-156`, `W/settings/scopedSettings.ts:43-414`, `W/settings/useScopedSettings.ts:43-100`, `W/settings/settingsScopeAxis.ts`, `W/settings/SettingsScopeSentence.tsx`, `W/settings/ScopedSwitch.tsx:12-19`, `W/ui/switch.tsx:12-46`, `W/settings/themeEditorStore.ts:42-68`, `W/settings/ThemeEditorHost.tsx:33-131`, `W/settings/ThemeEditorPanel.tsx:296-360, 1090-1230`, `W/CommandPalette.tsx:554, 2209`, `apps/web/src/routes/__root.tsx:253`, `W/settings/themeInspector.ts` (501 lines).
Library revision: `20261005-platforms-v3`. Selected topics: components (child state lives with the instance; keep the host mounted while a session is open), layout-and-interaction (a floating box above routes; pan gestures), state-and-data (several writes, one result), design (all states), accessibility (tri-state `aria-checked` is not documented; `aria-expanded`/`aria-pressed` are), motion (switch thumb 150 ms; reduced motion), testing-and-debugging, platforms (macOS resize is supported by the agent). Unknown in the library: `aria-checked="mixed"`, app-local Swift. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed: the clone's scope menu offers "All environments" and the focused environment (`settings-core.ts:150-153`), and a "All environments" write reaches only the focused one; `projectGroups()` reads the focused shell only (`client.ts:967-980`). The reference switch is 30×18 with a 14 px thumb; mixed moves the thumb to the centre, sets its opacity to 70 % and uses the unchecked track (`ui/switch.tsx:21-41`); a press on a mixed switch is a press on an unchecked one. The clone's editor lives inside Settings: it renders only while `settingsOpen` (`app-settings.contract:200-201`; `SettingsWindow` is mounted at `app.contract:1318`), the draft syncs only while Settings is active (`settings-core-view.ts:40`), and the palette row opens Settings (`app.contract:285-292`). Settings stays interactive during create, edit and duplicate (`app-settings.contract:163`) and the draft already paints the app through `look()` (`settings-appearance-look.ts:39-48`); keep both. Panel constants from the reference: default bottom-right 16, width min(416, window − 32), max height min(672, window − 96), margin 8, keep at least 48 of the header on screen, resize minimum 280×220, grow only toward the right and bottom, a minimized panel hugs its header. Drag precedent in Contract: `pan=` in `r5-shell.contract`, `sidebar-row.contract`, `settings-b-accent.contract`.
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (tooltip, popover and hover Contract).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and root-view room exist) | pending |
| recorded decision | Plan decision U18: theme editor "Inspect app colors" | none | User chooses: waive, or framework issue (X30) | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X30](../issues/20261005-x30-ts-announce-readback-picker.md), [X13](../issues/20261005-x13-hover-keys-during-pan.md), [X22](../issues/20261005-x22-reactive-layout-facts.md), [X11](../issues/20261005-x11-shadow-blur-parity.md), [X9](../issues/20261005-x09-root-component-across-files.md), [X8](../issues/20261005-x08-agent-pointer-native-views.md), [X21](../issues/20261005-x21-two-way-websocket.md), [X43](../issues/20261005-x43-tristate-switch-mixed.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X13 | Hover and keys during a pan | Header drag uses a pan; Escape during a drag | nonblocking | Test what Escape does mid-drag |
| X22 | Reactive layout facts | Clamp needs the window size on resize; the clone has a `viewport` resource (`viewport.height`) | nonblocking | Re-clamp from that resource |
| X11 | Glass panel and shadow | `dialog-glass` panel | nonblocking (visible difference declared) | Cite in the matrix |
| X9 | Root cap | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines | nonblocking until the cap | Keep the editor view model in the existing `settingsCore` resource (compute it while an editor dialog is open even if Settings is closed); no new resource; panel position and size are child state |
| X8 | Agent pointer input | Real drag and resize cannot be sent by the agent | nonblocking (workaround: pure-function tests + attended row) | none |
| X21 | RPC from a data module | One request per environment through its own transport | nonblocking | none |
| [X43](../issues/20261005-x43-tristate-switch-mixed.md) | `aria-checked="mixed"` on a switch | Documented attributes are `aria-expanded`, `aria-pressed`; the reference sets it at `ui/switch.tsx:33` | nonblocking (the sighted result is the same; the accessibility state differs and is declared) | Check at `prepare`; apply the X43 adoption steps when it lands |
| X30 | Pixel readback and colour provenance under the pointer (the app's equivalent of the DOM inspector) | The reference's Inspect walks the DOM for the theme role of the picked element (`themeInspector.ts`); X30 covers pixel readback, not the role lookup | blocking for the Inspect button only (decision U18) | Plan decision U18: waive Inspect, or open a framework issue through `issue-open` |

## Implementation notes

- Ports with headers and listed changes: `settings-scope.ts` (`settingsScope.ts` and the axis helpers), `scoped-settings-plan.ts` (`scopedSettings.ts`; `Equal.equals` becomes the clone's `canonical` comparison, `settings-core.ts:183`), `theme-editor-session.ts` (store functions; the session names themes by id so a saved definition is read fresh), `theme-editor-notices.ts` (the saved notices of `handleSaved`: "<label> created" / "It’s now active."; "<label> saved" / "Your changes are now active." or "Your changes are saved."; "<label> updated" / "Its <appearance> palette was added."; error "Could not save your theme" / "Browser storage is unavailable, so the change was not kept.").
- The settings view model takes the target list from a function that merges the focused client and `EnvironmentFleet.entries`; offline entries stay selected and unavailable, and the resolver never broadens a stale selection. The scope menu labels disambiguate same-name environments by address.
- Switch: add `mixed: bool` to the row shape; thumb at margin 6, opacity 0.7, unchecked track; the press sends the existing "true" value. Thumb transition `translate` 150 ms (reduced motion: none).
- Theme editor: root-level `ThemeEditorHost` in `app.contract`'s view next to `SettingsWindow` (one line); the `theme-editor` op toggles the session without setting `settingsOpen`; Settings › Appearance buttons use the same op. Header drag ignores presses on buttons, inputs and links. Close on Escape (`aria-keyshortcuts="Escape"` exists).
- States: scope unavailable (message, writes refused), environment disconnected ("Reconnect the selected environment to change this setting."), read-only session, mixed, saving, error text; editor default, minimized, dragged, resized; hover and keyboard focus on header buttons and switches. `aria-label`: "Minimize the theme editor" / "Expand the theme editor", "Close the theme editor", "Filter colors", "Use advanced theme colors", "Theme appearance", each switch's title.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Scope resolution | bun | Run the ported tests | Same kinds, labels, members and unavailable reasons as the reference; a removed target never broadens | macOS host machine | test log |
| Environments listed | Two lane backends (ports 16000–16999), both switched on | `bun scripts/agent.mjs macos --json tree state "tap settings-scope-environment" tree` | Menu lists both (disambiguated when names match); offline one disabled | macOS, 1280×840 and 840×620 | transcript, screenshots |
| Write to all | Same, "All environments" | Toggle `enableProviderUpdateChecks` | Two `server.updateSettings` requests, one per transport; both servers show the value (`server.getSettings`); no write to a third environment | macOS | trace, settings dumps |
| Partial failure | Same; stop one backend mid-write (recorded PID only) | Toggle again | Error toast "Setting saved on some environments" naming the stopped environment; the other saved | macOS | transcript |
| Mixed across environments | Same, values differ | Open the row; press it | Mixed look (thumb centred, 70 %), `inheritance: mixed`; one press turns both on; not mixed after; selects show "Mixed" | macOS | transcript, trace |
| Mixed across checkouts | One environment, project with two checkouts, overrides differ for `newWorktreesStartFromOrigin` | `layout setting-start-from-origin`; press | Same result per checkout | macOS | transcript |
| Non-mixed unchanged | Equal values | Press | On and off as before | macOS | transcript |
| Editor lifetime | Fixture thread list | Palette › Toggle theme editor; open another thread; open and close Settings; press the shortcut | Panel stays across all of it with its draft and the app painted with it; the shortcut closes it; Settings is not opened by the palette row | macOS, both sizes | transcript, screenshots |
| Save notices | Create, edit, merge a missing half, remove the theme while editing | Save | Notices as above; a removed theme turns the save into a create | macOS | transcript |
| Clamp and minimize | — | Agent window resize to 840×620 and back with the panel open and minimized | Panel within margin 8, header reachable; size shrinks before the position is clamped | macOS | transcript, `layout` |
| Real drag and resize `(attended session)` | Real pointer; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Drag the header (grab cursor), drag the corner grip, drag past edges, press header buttons | Panel follows; clamps as specified; header buttons are not dragged; Escape closes | macOS, real input | session notes |
| Inspect `(blocked by X30 / decision U18)` | — | Press Inspect, pick an element | Expected: the picked colour's role is selected and "N uses" shows | macOS | open until decision U18 waives it or the framework issue is resolved |
| Trace and pixels | Oracle and clone on the same lane backends | Per-environment write payloads; pairs at 1280×840 and 840×620, light and dark, for mixed switch, scope menu, panel default and minimized | Same `server.updateSettings` writes; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | diff, images |
| Ported tests | — | `bun test` | `settingsScope.test.ts` ("settings scope search" 3, "settings scope resolution" 8), `settingsScopeAxis.test.ts` (6), `scopedSettings.test.ts` (selection `:104`–`:124`, targets `:131`, writes `:193`–`:278`, `:346`–`:405`, "scoped settings mixed values" `:431`–`:439`, overrides `:470`–`:561`), `themeEditorStore.test.ts` "toggleThemeEditorForTheme" (2), `ThemeEditorHost.test.tsx` ("reopens the same %s with its saved colors", "refreshes an open %s when the library changes", "does not keep editing a theme removed from the library" as logic tests on session and library); original names | macOS host machine | test log |
| Keyboard focus, Escape, reduced motion | Two lane backends; editor session open | Tab to the scope menu; Return; arrow keys; Return; Escape; Tab to a switch; Space; open the theme editor and Tab through its header buttons; Escape; `prefer prefers-reduced-motion reduce` then press a switch and minimize the panel | The menu opens and closes by keyboard and returns focus to its trigger; Space turns a mixed switch on; Escape closes the editor; header buttons show a focus ring; the thumb move and the minimize turn are instant under reduced motion | macOS | transcript |
| Gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/settings-scope.ts`, `C/scoped-settings-plan.ts`, `C/theme-editor-session.ts`, `C/theme-editor-notices.ts`, `C/settings-core.ts`, `C/settings-core-view.ts`, `C/settings-rows.contract`, `C/source-control-view.ts`, `C/settings-rest-commands.ts`, `C/settings-b-fleet.ts` (read access only), `C/settings-appearance-editor.ts`, `C/settings-appearance-editor.contract`, `C/app-settings.contract`, `C/app.contract` (view line, op), `C/palette.ts`, `C/keyboard-dispatch.ts`, their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, two isolated lane backends. Never port 3773, `~/.t3` or the `t3code` scheme; stop only recorded PIDs. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

Implemented on `feat(example)/t3-code-settings-scoped-controls-and-theme-editor` (2026-10-06), rebased on
`feat(example)/t3-code` `da40e6590` (after hot-file-split #147). Verification: unverified (no independent review;
attended rows not run).

- `settings-scope.ts`: ports of `validateSettingsScopeSearch`, `resolveSettingsScope` and the axis helpers
  (`settingsScopeEnvironmentLabel`, `environmentAxisValue`, `projectAxisValue`, `selectEnvironmentAxis`,
  `selectProjectAxis`, `selectSingleEnvironmentScope`). A member's `physicalProjectKey` is the clone's checkout id
  (the project id `settingsCheckout` carries).
- `scoped-settings-plan.ts`: ports of `selectScopedSettingsEnvironments`, `resolveScopedSettingsTargets`,
  `scopedSettingsAreMixed`, `scopedSettingsSource`, `planScopedSettingsPatch`, `planScopedSettingsClear`,
  `listProjectOverrides`, `planProjectOverridesClear`, `persistScopedSettingsPatch`, the `projectSettings.ts` helpers
  they call, and useRunScopedPlan's toasts (`scopedPlanNotice`).
- `settings-scope-sources.ts`: the scope's environments (the focused client plus every fleet entry, offline ones kept
  and marked), project groups joined across environments, one `server.updateSettings` per environment through its own
  transport (`EnvironmentFleet.native`), and t3.json reads per member through its environment.
- `settings-core.ts`: `resolveScope` returns the ported resolution; `serverState` reads every connected target
  (mixed, source); `settingPatch` is replaced by `settingPlan`; `applyCoreSetting` plans, writes all targets and toasts
  "Setting not saved" (warning) or "Setting saved on some environments" / "Setting not saved" (error naming the
  environments). "Continue threads after restarts" needs every selected connected environment's capability; Advanced
  background activity needs one selected environment.
- D15: `mixed` on `CoreRow` and `ScopedRow`; `settings-scoped-switch.contract` `ScopedSwitch` (thumb `translate`
  150 ms, none under reduced motion; mixed thumb at 6 pt, 70 %, unchecked track; a press on mixed sends "true") for the
  seven General switches and the two Integrations device rows. The Integrations page still resolves one environment
  and one checkout, so its rows are never mixed today.
- D16: `theme-editor-session.ts` (store, `toggleThemeEditorForTheme`, session themes read fresh by id),
  `theme-editor-notices.ts` (handleSaved), `settings-appearance-editor.ts` (the draft belongs to the session; a removed
  theme saves as a create; a create named like a light- or dark-only theme adds the missing palette),
  `ThemeEditorHost` at the window root (`app.contract` one view line), the palette row and `themeEditor.toggle` toggle
  the editor without opening Settings, opening Settings keeps the session; the panel drags by its header, resizes from
  its corner grip (280x220 minimum, toward right and bottom), clamps to margin 8 with the header reachable, and is
  pulled back into view after a window resize.
- Follow-up for the user's review (#153 matches the original T3 Code app): a new theme saves only the palette of
  the appearance being edited, and an edit keeps the theme's own palettes; every Create, Edit or Duplicate request is a
  new session (the window numbers it in the dialog subject), so Create while a create is open restarts the draft;
  after the window shrinks and grows the panel keeps its clamped place (a window-sized tracker's `resize=` runs the
  reference's clamp); the Integrations Device hub and Agent device access switches resolve the settings scope's
  targets, draw mixed and write every selected environment (`settings-integrations-scope.ts`).
- Not done: Inspect (X30 / U18).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | see PR | `bun test examples/t3-code` 1383 pass / 0 fail (base `da40e6590`: 1321); strict tsc clean; `contract build` 2195 slots, 43 resources, 48348 nodes; `cargo test -p t3-code-macos --lib` 10 pass; no Swift changed (no AppKit binary touched); `git add -A && bun scripts/caps.mjs` within caps; five checks green (`cargo test` 2926 pass, 0 fail, 18 ignored; clippy, fmt, caps, boot exit 0); macOS bundle built | Before/after agent drives, see below | attended rows (real drag and resize, VoiceOver mixed state X43); Inspect (X30, U18); oracle and trace-diff not run (desktop-oracle-and-trace not built) |

Live drives (2026-10-06, `scripts/agent.mjs macos --size 1280x840`, one call each, lane servers `1e2ecbd975` on
127.0.0.1:16100-16103 with isolated homes, telemetry off; the app paired to server A in the welcome wizard and added
server B in Settings › Connections). BEFORE = `t3-code-evidence-base` at `da40e6590`, AFTER = this branch at `c0e31d9`.

- Scope menu: before lists All environments and one environment; after lists All environments and both, disambiguated
  by address (`… · http://127.0.0.1:16102`, `… · http://127.0.0.1:16103`).
- Mixed: after, B selected (`settings-scope-choice-environment-<B>`), Auto-resume limited threads toggled on B only,
  then All environments: the row's track is `light-dark(#d4d4d8, #ffffff14)` (unchecked) with the thumb centred; one
  press turned it on. Server files after the press: A `"autoResumeLimitedThreads": true`, B `true`. Before, the same
  All-environments press wrote A only (B has no `settings.json`).
- Theme editor: Settings › Appearance › Create theme, then Close settings: before the editor closes with Settings;
  after it stays over the app. Header drag -420,-260: `left = 428`, `top = 216` (from 848,476); window 840x620:
  `left = 416` (= 840 - 416 - 8), `max_height = 524` (620 - 96); back at 1280x840 `max_height = 672`; minimized
  `max_height = 840`; reopening Settings keeps it (`layout theme-editor` answered); `Meta+Alt+Shift+t` closed it
  (final `tree` has no `theme-editor` and no `settings-dialog`).
- A first AFTER run showed the drag starting at the window origin (`left = 8`, `top = 8`): `frame()` reads by view
  id and the panel had only a testId; fixed in `c0e31d9`, the second run above.
- Not run: real-pointer drag and corner grip (attended), VoiceOver on a mixed switch (X43), oracle and trace-diff
  pairs (desktop-oracle-and-trace not built), two-checkout mixed state live (the lane server shows "No projects yet";
  covered by `settings-scope-sources.test.ts`), partial failure live (covered by tests).

Follow-up drive (2026-10-06, one BEFORE on `t3-code-evidence-base` `ea18e1f`, one AFTER on `0c71af3`; servers on
127.0.0.1:16100-16101, B seeded with `enableDeviceSupport: true`): Create theme, name "Half done", Create theme again:
before the name stays, after the draft restarts (empty name). After: drag to `left = 428`, 840x620 `left = 416`,
1280x840 again `left = 416` (before this change it returned to 428). The Integrations Devices section was below the
fold in both shots, so the mixed device switch is proven by `settings-integrations-scope.test.ts` only.

## Next action

Review the PR; run the attended rows (real header drag, corner grip, VoiceOver on a mixed switch).
