---
name: 20261009-settings-escape-and-nav
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-escape-and-nav
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

## Cause and fix

The root cause of S1-1, S1-2, S1-3 and S2-1: an `aria-keyshortcuts` button hears its chord before any `key` handler
(docs/contract-grammar.md "Shortcuts"), and among several the host presses the lowest node (`ShortcutsMac.perform`:
`nodes().first`), so Settings' Back took every Escape unless `menuOpen` named the owner, and it named only some. The web's
order differs (the page's Escape runs only when nothing prevented it), so each owner has to be known at the window.

- `app-settings.contract` `SettingsWindow`: one `derive escapeOwned` (the old `menuOpen` terms, plus the Keybindings
  search, a recording shortcut, the licence search, and an open "Add custom model" field) feeds Back's `menuOpen`. The
  "Add custom model" field's open state moved from `ProviderModels` into the window (`modelAdding`, set through the
  Providers page's `ui("model-adding", id)`; a route change or another provider clears it, as the reference's section
  unmounts); its Cancel keeps its Escape shortcut, now the only one heard. The slug field and the Keybindings search have
  `autofocus` (the reference's `autoFocus`); both searches `preventDefault()` their Escape.
- The When editor and the font list are `aria-modal` popovers (the host's frontmost-modal rule, `Shortcuts.swift`): no
  shortcut outside them hears a key while they show, so the host's light dismiss closes them on Escape (the theme colour
  picker's pattern). A Settings dialog (`restEditor`, `settingsCore.saOpen`: Add device host and the confirms, the
  archive confirm) makes `settings-dialog` inert, so the dialog's own Escape (Close, Cancel) is the only one heard.
- The recorder's row is `aria-modal` while it records, so it takes every chord (Back's Escape, ⌘K, "/"), and
  `ShortcutDispatch` gets no items while it records on the Keybindings route, so no ⌘ menu item can run either (the
  reference's shortcut listeners skip `[data-keybinding-capture]`). Leaving the recorder (`blur`) stops recording and keeps
  the draft (the reference input's `onBlur`); `captureKeybinding`'s `"blur"` keeps `keybindingKey`. The conflict warning
  now has the reference's tooltip sentence (`WarningTooltipIcon`), and the unknown-condition warning its own.
- S2-5: the Settings nav takes the shared `sidebarWidth` (clamped as AppSidebarLayout's: 13rem or the brand, up to the
  window less 40rem), not `min(sidebarWidth, viewport / 5)`, and `SettingsWindow` mounts the thread sidebar's
  `SidebarWindowRail` (Resize Sidebar: drag, double-click reset) with its "Drag to resize sidebar" tip.
- `EXACT2-GAPS.md` "Escape inside Settings" is corrected to this mechanism.

## Acceptance results

Before = feature tip `950e8e2e5` (`t3-code-evidence-base`), after = this branch's bundle (`527ec4c2e`, one live
agent-mode drive, 135 ops), reference = T3 Code `1e2ecbd975` Electron over CDP. Each image: before | after | reference.

| Id | Result | Proof |
| --- | --- | --- |
| S1-1 | pass (agent): the Keybindings search clears and closes (79 bindings, Keybindings stays); the When popover closes and the new row stays; the font list closes and Appearance stays; a second Escape with nothing open leaves Settings | [s1-1-keybinding-search.png](https://raw.githubusercontent.com/ccheever/exact2/d2b0760601310adc16d4a89b40b79f5e90b93dbf/settings-escape-and-nav/s1-1-keybinding-search.png), [s1-1-when-editor.png](https://raw.githubusercontent.com/ccheever/exact2/6bee534636d4d32110e5c40d230be7f1e159c161/settings-escape-and-nav/s1-1-when-editor.png), [s1-1-font-search.png](https://raw.githubusercontent.com/ccheever/exact2/b7cb7928ca70aed24977a12fccaa69515c672e82/settings-escape-and-nav/s1-1-font-search.png), [s1-1-second-escape.png](https://raw.githubusercontent.com/ccheever/exact2/ad3854e4f08605ded3c7cadc56fe519b7fee7fb0/settings-escape-and-nav/s1-1-second-escape.png) |
| S1-2 | pass (agent): Escape in the slug field cancels "Add custom model"; Providers stays. `EXACT2-GAPS.md:386` corrected | [s1-2-custom-model.png](https://raw.githubusercontent.com/ccheever/exact2/cf981cd7e4b2c8b73da0243e9f78c81bc196ea87/settings-escape-and-nav/s1-2-custom-model.png) |
| S2-1 | pass (agent): Escape in New task's Name and in Add device host's Name closes only the dialog. Before, the closed New task left `restEditor` set, so the whole window stayed inert ([drive record](https://raw.githubusercontent.com/ccheever/exact2/14cd2fa3001434a50cd405d7b00263b5518854fb/settings-escape-and-nav/drive-record.txt)) | [s2-1-new-task.png](https://raw.githubusercontent.com/ccheever/exact2/1043e8f0512604af6a867809a6614d6d409bec42/settings-escape-and-nav/s2-1-new-task.png), [s2-1-add-host.png](https://raw.githubusercontent.com/ccheever/exact2/136e37b1cf4e5b55beeac43bc8e57dc21e1e4a8e/settings-escape-and-nav/s2-1-add-host.png) |
| S1-3 | pass (agent, the agent's Escape through the platform route into the native recorder): recording cancels, ⌘N is back, Keybindings stays. Real Escape: open (real-input step 1) | [s1-3-recorder-escape.png](https://raw.githubusercontent.com/ccheever/exact2/96e660b21f124fc1aec52a1e87010229bed3157f/settings-escape-and-nav/s1-3-recorder-escape.png) |
| S1-4 | pass (agent, the agent's ⌘K): the field reads `mod+k`, the warning "Conflicts with Command Palette: Toggle." with the reference's tooltip sentence, Save enabled, no palette (tree in the drive record). Real ⌘K: open (real-input step 2) | [s1-4-record-bound-chord.png](https://raw.githubusercontent.com/ccheever/exact2/4366c0a7ff1078103b1b83e2535f36c0a6282444/settings-escape-and-nav/s1-4-record-bound-chord.png) |
| S2-5 | pass (agent drag with the mouse from the nav's edge): the rail is at x 247-263, a 120 pt drag widens the nav 257 → 377 (reference 256 → 376), the tip shows on hover, double-click resets. Real drag: open (real-input step 3) | [s2-5-settings-rail.png](https://raw.githubusercontent.com/ccheever/exact2/63ce07cb69225494731bfd4736d5a9ca13137625/settings-escape-and-nav/s2-5-settings-rail.png) |

Drive scripts: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/4e9c607297ced3a4f6df887f58d1f2262ab20d2a/settings-escape-and-nav/drive.sh.txt) (both builds, same steps;
the before build needed three sessions because of its S2-1 bug), [ref-flow.mjs](https://raw.githubusercontent.com/ccheever/exact2/9c3b1d1cb507624a715682cf1a119779ddad03f1/settings-escape-and-nav/ref-flow.mjs.txt).

Tests: `settings-escape.test.ts` (10 tests: every Escape owner in `escapeOwned`, the modal popovers and recorder, the
inert page under a dialog, the window's "Add custom model" state, ShortcutDispatch off while recording, the warning
tooltip, the shared width and the rail); `theme-color-picker.test.ts` still pins Back's line. Checks: see the PR.

## Real-input batch steps

Build this branch's bundle (`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`) and
launch it on the audit lane, not agent mode: `T3_LOCAL_HOME=<A>/lanes/settings-escape-and-nav/clone-t3-home
T3_LOCAL_PORT=16902 EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle --run`
(`<A>` = `target/t3-audit` of the base checkout; never port 3773 or `~/.t3`).
1. **S1-3.** Settings › Keybindings, click the ⌘N chips of the first "Chat: New" (the field reads "Press shortcut"),
   press Escape. Expect: the chips read ⌘N again and Keybindings stays. Press Escape again: Settings closes.
2. **S1-4.** Settings › Keybindings › + › Command: Diff: Toggle › click "Unassigned", press ⌘K. Expect: the field reads
   `mod+k`, an amber triangle (hover: "Conflicts with Command Palette: Toggle. The most recent matching binding wins
   when both conditions can apply."), Save enabled, no command palette. Click "Unassigned"/the field again, then click an
   empty part of the page: recording stops and the field keeps its text. Do not press Save.
3. **S2-5.** Settings › General. Hover the nav's right edge: the cursor is a resize arrow and "Drag to resize sidebar"
   shows. Drag the edge 120 pt right: the nav is about 376 pt wide. Back (or Escape): the thread sidebar has the same
   width. Settings again, double-click the edge: 256 pt.

## Next action

Review the draft PR. The three real-input steps above (S1-3 real Escape, S1-4 real ⌘K, S2-5 real drag) go into the next
real-input batch; every row passes in agent mode.
