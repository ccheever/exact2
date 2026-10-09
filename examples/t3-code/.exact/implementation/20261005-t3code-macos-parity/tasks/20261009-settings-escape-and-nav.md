---
name: 20261009-settings-escape-and-nav
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Settings: Escape goes to its owner, the shortcut recorder owns every key, and the Settings sidebar resizes

## Outcome

- Escape in Settings closes the innermost thing that owns it, as in the reference:
  - a field's popup, the When editor, the font list, the custom model editor;
  - a Settings dialog (New task, Add device host);
  - a recording shortcut.
  - It leaves Settings only when nothing owns it.
- While the keybinding recorder records, a bound chord (⌘K) is recorded with its conflict warning, not run.
- The Settings sidebar has the "Resize Sidebar" rail.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

S1-1, S1-2 and S2-1 have one root cause (Settings' Back keeps its Escape shortcut while something inside owns Escape). The
audit found it in two areas. This task fixes it once.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| S1-1 | Escape in Keybindings' search clears the query and closes the field; the page stays (78 bindings). Escape in the When editor's expression field closes the When popover; the new row stays. Escape in the Interface font family search closes the font list; the page stays. | In all three, Escape leaves Settings and shows the thread view (the field keeps its text; a new keybinding row is discarded). | 1. Keybindings › Search keybindings, type "diff", Escape. 2. Keybindings › Add keybinding › When (Always) › + Condition, focus the expression, Escape. 3. Appearance › Interface font family, type "SF" in Search fonts, Escape. | `target/t3-audit/evidence/settings-1/S1-1-ref.png`, `S1-1-clone.png`, `S1-whenesc-ref.png`, `S1-whenesc-clone.png`, `S1-fontesc-ref.png`, `S1-1-fontesc-clone.txt` |
| S1-2 | Escape in the custom model slug field cancels the editor; the page stays on Providers. | Escape closes Settings. `EXACT2-GAPS.md:386` says an open custom model editor makes Back give up Escape (`providerPage.escapeOwned`); on this drive Back still took it (regression). | Providers › Add custom model; with the focus in the slug field, Escape. | `target/t3-audit/evidence/settings-1/S1-2-ref.png`, `S1-2-clone.png` |
| S2-1 | Escape closes only the dialog; the page stays (`#/settings/scheduled-tasks` or `#/settings/integrations`). A second Escape leaves Settings. | One Escape (focus in the dialog's Name field) closes the dialog and Settings. Same for Add device host. Add Environment is right, because the overlay is inert while "connecting". | Scheduled Tasks › New task, Escape in Name. Or Integrations › Add host, Escape in Name. | `target/t3-audit/evidence/settings-2/S2-1-ref.png`, `S2-1-clone.png`, `S2-1b-ref.png`, `S2-1b-clone.png`, `settings-2/clone/d7-full.tree.txt` |
| S1-3 | Edit shortcut for Chat: New shows "Press shortcut"; Escape cancels recording and the page stays. | The recorder shows "Press shortcut"; Escape (sent by the agent through the platform path) closes Settings. | Keybindings › click the ⌘N chips of Chat: New › Escape. | `target/t3-audit/evidence/settings-1/S1-3-ref-recording.png`, `S1-3-clone-recording.png`, `S1-3-clone.png` |
| S1-4 | New keybinding, command Diff: Toggle, focus the recorder, ⌘K: the field reads "mod+k", a warning shows "Conflicts with Command Palette: Toggle. The most recent matching binding wins when both conditions can apply.", Save is enabled. | ⌘K opens the Command Palette over Settings; nothing is recorded. The app's shortcut buttons (ShortcutDispatch, `app-window.contract:590`) stay on while `keybindingRecording` is true. | Keybindings › Add keybinding › Command › Diff: Toggle › click the shortcut field › ⌘K. | `target/t3-audit/evidence/settings-1/S1-4-ref.png`, `S1-4-clone.png` |
| S2-5 | The Settings sidebar has a "Resize Sidebar" rail; a 120 px drag widened the nav from 256 to 376 px. | No rail in `#settings-navigation`. The width is `max(208, min(sidebarWidth, viewport/5))`, so at 1280 px it cannot pass 256 px. | Open Settings; drag the settings sidebar's right edge. | `target/t3-audit/evidence/settings-2/S2-5-ref.png`, `S2-5-clone.png` |

## Scope and exclusions

Included: the six findings above.

Excluded:
- Framework code. Capture-phase keys wait for main fix of [#140](https://github.com/ccheever/exact2/issues/140) (main file
  `issues/20261009-capture-phase-key-events.md`). If S1-3 needs the recorder to see Escape before Back's shortcut and
  the clone cannot order that, record it for the coordinator and keep the row open.
- Other Settings rows: [settings-rows-and-labels](20261009-settings-rows-and-labels.md).

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`): `hooks/useNavigateBack.ts:19-41` (the page's Escape runs only if
nothing called `preventDefault`; dialogs, popovers and fields consume it); `routes/settings.tsx` (Settings renders inside
the app Sidebar, with its rail); the keybinding recorder and its conflict text in the Keybindings settings components.

Clone (`examples/t3-code`):
- `app-settings.contract:211` (`settings-dialog` inert omits `restEditor`/`hostEditor`), `:213` (`SettingsNav` `menuOpen`
  omits the keybinding search, the When editor, the font picker, `restEditor` and the host editor; the width expression;
  no rail).
- `settings-core.contract:375` (Back's `aria-keyshortcuts=Escape`); `settings-scheduled.contract:325` (the dialog Close
  also declares Escape).
- `providers-upkeep.ts:160` (`escapeOwned` from `upkeep.modelEditor`), which feeds `menuOpen`.
- `app-window.contract:590` (ShortcutDispatch); the `t3-key-recorder` hatch (`settings-keybindings.contract`, `app.json`).
- The main sidebar's rail (`resizeSidebar`, `finishResize` in `app-window.contract`) is the pattern for S2-5.

Fix shape: derive one "Escape is owned" value for Settings from every owner above. When it is true, Back gives up Escape.
When the recorder records, switch off ShortcutDispatch, so the recorder takes every chord.

Shared file: `app-window.contract` with [app-color-scheme](20261009-app-color-scheme.md) (line 580). Rebase on whichever
merges first.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| S1-1 | Each of the three Escapes closes its field's popup or clears the search; Settings stays. A second Escape (nothing open) leaves Settings. | `s1-1-keybinding-search.png`, `s1-1-when-editor.png`, `s1-1-font-search.png` | agent |
| S1-2 | Escape in the slug field cancels the editor; Providers stays. Correct `EXACT2-GAPS.md:386` if the mechanism changes. | `s1-2-custom-model.png` | agent |
| S2-1 | Escape in New task and Add device host closes only the dialog. | `s2-1-new-task.png`, `s2-1-add-host.png` | agent |
| S1-3 | Escape while recording cancels the recording; Settings stays. | `s1-3-recorder-escape.png` | agent, then needs_real_input (a real Escape into the native recorder) |
| S1-4 | ⌘K while recording reads "mod+k" with the conflict warning; Save is enabled; no palette opens. | `s1-4-record-bound-chord.png` | agent, then needs_real_input (real ⌘K) |
| S2-5 | The rail shows; a 120 px drag widens the nav as the reference (agent `tap … drag`). | `s2-5-settings-rail.png` | agent drag, then needs_real_input (a real drag) |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build and unit-test the "Escape owned" value. Then do one batched live
drive at the end for every row's before/after pair, and the real-input rows when the screen is unlocked. Close every row
in this PR, or record the blocker of a row that cannot pass.
