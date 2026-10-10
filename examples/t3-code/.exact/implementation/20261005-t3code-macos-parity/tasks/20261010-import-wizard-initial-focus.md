---
name: 20261010-import-wizard-initial-focus
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-import-wizard-initial-focus
pr_url: https://github.com/ccheever/exact2/pull/409
verified_commit: null
---

# The browser import wizard's initial focus

## Outcome

When the browser import wizard opens on Configure, T3 Code (`1e2ecbd975`) focuses its first tile ("Personal / 5
cookies"). `BrowserImportWizard.tsx:145-146` is a Base UI `Dialog` / `DialogPopup` with no `initialFocus` and no
`autoFocus`, so the popup's first tabbable element takes the focus, and the close X comes after the step
(`ui/dialog.tsx` `DialogPopup`). Base UI focuses only when the dialog opens, not when a step changes. The clone focuses
Import (`browser-profiles.contract`, `BiButton(buttonId="browser-import-run", … first=true)`, `autofocus=first`). Its
other steps each autofocus a button ("I’ve quit it", Cancel on Full Disk Access, Done, Close), and `autofocus` applies
again at every step change. Found by [realinput-1010e-followups](closed/20261010-realinput-1010e-followups.md) RE-4 (#406).

## Steps

1. On the live reference over CDP (`target/t3-audit/ref-app.sh`, the lane's browser fixture, never real browser data),
   open the wizard from Settings › Integrations › Add profile › Chrome and record `document.activeElement` when it
   opens and after each step change (Configure, the quit prompt, Full Disk Access, running, done, an error).
2. Make the clone's wizard focus the same element on open, and keep the focus where the reference keeps it on a step
   change (do not move it unless the reference does). The removal confirmation's Cancel (`browser-profile-remove-cancel`)
   is a separate dialog: compare it too.
3. A test of the focus target per step; one agent drive reading the focused node on open and after a step change.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Focus on open and per step matches the reference | CDP record of the reference; agent `tree` focus on the clone | text before / after / reference, an image of the open wizard |
| Focus ring under real keys | a real-input step for the next session | — |

## What the reference does (CDP, lane `import-wizard-initial-focus`, ports 16680/16681)

The lane's fixture browsers only: the realinput-1010e lane's stores, plus Brave held by a `SingletonLock` naming another
host (so it reads as running) and a Firefox profile "broken" whose `cookies.sqlite` is not a database. Only Firefox was
imported (no Keychain read). `document.activeElement`, step by step ([record](https://raw.githubusercontent.com/ccheever/exact2/28d74170d767434f4768e7589f7b164550e5b8ec/import-wizard-initial-focus/focus-record.txt)):

- **Opening** (Base UI `initialFocus`: the popup's first tabbable element, `FloatingFocusManager` 340-388): Configure
  (Chrome, Firefox) the first "From" tile, by pointer (no ring) and by keys (ring); the quit step (Brave) Cancel. Full
  Disk Access cannot be reached on the lane (its check needs TCC's `EPERM`, `SafariCookies.ts` `isPermissionDenied`):
  by the same rule it is `PermissionChecklist`'s Allow, the popup's first tabbable element.
- **A step change** removes the focused button with its step's component; `DialogPopup` passes `restoreFocus: "popup"`,
  so the popup itself takes the focus: right after "I’ve quit it" (Checking) and after the recheck (Quit again), right
  after Import (Importing), after the failed import (Couldn’t import), after Try again, after a successful import (Done).
  From there Tab goes to the screen's first stop (Cancel, Close, Done) and Shift+Tab, through the focus trap's guard, to
  its last (the X).
- **Closing**: Cancel, Escape, Close all give it back to Add profile (RE-4, #406).
- **Remove “<name>”?** (AlertDialog, opened from the row's menu): Cancel, by pointer and by keys; Cancel or Escape gives
  it back to the row's options button ("Firefox options"); after "Remove profile" the button is gone and Base UI falls
  back to the last element that held the focus as a popup opened (`getPreviouslyFocusedElement`): Add profile in the run.

## What the clone did (evidence base `c03d7e908`, agent mode)

Every opening left the focus on the control behind the dialog: the menu gives the focus back to Add profile (or the
row's options button) as it hides, so the buttons' `autofocus` never took it (an `autofocus` takes the focus only when
nothing holds it). A step change removed the focused button and the next step's `autofocus` took it (I’ve quit it,
Close, Done); Shift+Tab from Close went to a button of the Settings page behind the dialog (`device-hosts-add`). Cancel
on the confirm left the focus nowhere.

## Fix

- `browser-profiles-settings.ts`: `wizardTabStops` lists each screen's Tab stops in tree order (the enabled tiles and
  buttons, then the X); the view carries `openFocus` (the first stop; on Full Disk Access already granted, the popup, as
  the grant replaces Allow), `tabFirst`, `tabLast`, and `focusKey`, which changes with each change of screen (`goTo`, at
  every place the step is set; a step of the same kind keeps its screen) and with a grant on Full Disk Access (Allow is
  replaced). The view also carries `removalId`.
- `browser-profiles.contract`: the popup is focusable (`id="browser-import-popup" tabindex=-1`); the tiles, Allow and
  the X have ids; no button autofocuses (`BiButton` lost `first`). While the popup itself holds the focus, its `key`
  handler sends Tab to `tabFirst` and Shift+Tab to `tabLast` (EXACT2-GAPS X76: the Mac's key-view loop has no way on
  from a box that is no Tab stop). The rows' options buttons have ids.
- `app.contract` (RE-4's `importWizardFocus` kept as it was): `importWizardStepFocus` focuses `openFocus` as the wizard
  opens and the popup after each change of `focusKey`; `removalFocus` focuses the confirm's Cancel as it opens, then the
  row's options button, or Add profile when the row is gone.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Focus on open and per step matches the reference | pass (agent drive, reference over CDP): on open, Configure → the first "From" tile (before: Add profile, behind the dialog), Quit → Cancel (before: Add profile), Full Disk Access → Allow (before: Add profile; reference from source), the confirm → Cancel (before: the row's options button); after a step change → the popup itself (before: I’ve quit it, Close, Close, Done), Tab → Cancel and Shift+Tab → the X as the reference (before: the X, and a Settings button behind the dialog); the confirm's Cancel → the row's options button (before: none), its Remove → Add profile (before: none). Every row equals the reference's | [open-configure.png](https://raw.githubusercontent.com/ccheever/exact2/dd37602a9cfd67b9b8a7ebb78f0d36868af8906d/import-wizard-initial-focus/open-configure.png), [open-quit.png](https://raw.githubusercontent.com/ccheever/exact2/33392861aa9350cc15f7f603d2d4fad109b1ae5b/import-wizard-initial-focus/open-quit.png), [open-fda.png](https://raw.githubusercontent.com/ccheever/exact2/d71f45e6613d4a3a8d6c707e01acb1cc69ca489f/import-wizard-initial-focus/open-fda.png), [step-change.png](https://raw.githubusercontent.com/ccheever/exact2/268a9d483c46e3d720e043eefb588883a329f5db/import-wizard-initial-focus/step-change.png), [remove-confirm.png](https://raw.githubusercontent.com/ccheever/exact2/0546a0a011bbf8a9b4ab41503dc1de7ac35b160b/import-wizard-initial-focus/remove-confirm.png), [focus-record.txt](https://raw.githubusercontent.com/ccheever/exact2/28d74170d767434f4768e7589f7b164550e5b8ec/import-wizard-initial-focus/focus-record.txt) |
| Focus ring under real keys | implemented, not verified (real input): the agent's window is never key, so AppKit draws no ring in its screenshots (docs/agent-pitfalls.md); the rings follow the host's `:focus-visible` rule (after keys, not after a pointer press), as the reference's. Open until real-input step 1 runs | — |

## Real-input batch steps

Launch this branch's bundle normally as a lane copy (its own name and bundle id), with `T3_BROWSER_IMPORT_HOME` naming the
lane's fixture home `target/t3-audit/lanes/import-wizard-initial-focus/browser-home` (Brave reads as running through its
`SingletonLock`; never the account's browsers). Use Accessibility Inspector for the focused element.

1. **Rings and Tab.** Settings › Integrations › Browser profiles. (a) Click Add profile, click Chrome: "Personal" is
   focused with no ring. Press Tab: the ring moves to "Work". Escape. (b) By keys: Tab to Add profile, Return, ↓ to Chrome,
   Return: the ring is on "Personal". Escape: the ring is on Add profile. (c) Add profile › Brave: the ring-less focus is
   on Cancel; click "I’ve quit it": after the check the dialog is focused with no ring on any button; press Tab: the ring
   is on Cancel; Shift+Tab twice: the ring is on the X. Escape. (d) Add profile › Firefox, click "broken", click Import:
   "Couldn’t import" with no ring; Tab: the ring is on Close; Tab, Tab: Try again, then the X (a further Tab leaves the
   dialog: X53, not this task). Escape. (e) A profile of your own (Blank profile): its "…" menu › Remove profile and
   data: the focus is on Cancel; Escape: the ring is on its "…" button.

## Tests

- `import-wizard-initial-focus.test.ts` (new, 8): each screen's Tab stops in order and the focus as it opens
  (`wizardTabStops`, `wizardOpenFocus`), checked against the screens of `browser-profiles.contract` in tree order with
  their ids; what counts as a step change (`goTo`, `wizardFocusKey`: blocked → importing → blocked changes it, Configure
  set again does not, a grant does); the popup's Tab and Shift+Tab; no `autofocus` in the dialogs; the root's two tasks.
- `browser-profiles.test.ts`: the confirm's view carries the profile's id while open and none after.
- `realinput-1010e-followups.test.ts` (RE-4's task, unchanged) and `realinput-1010c-fixes.test.ts` pass as they were.

## Attempts and evidence

| Attempt | Commit | Result | Proof |
| --- | --- | --- | --- |
| Reference over CDP | T3 Code 1e2ecbd975 | every step recorded twice (the second with element rects for the images), Shift+Tab and Tab from the popup, the confirm's Cancel, Escape and Remove | [ref-focus.mjs](https://raw.githubusercontent.com/ccheever/exact2/a10c8d5b234409b51e37a37a6770357b53d4d93c/import-wizard-initial-focus/ref-focus.mjs.txt), [focus-record.txt](https://raw.githubusercontent.com/ccheever/exact2/28d74170d767434f4768e7589f7b164550e5b8ec/import-wizard-initial-focus/focus-record.txt) |
| Before drives (agent) | `c03d7e908` (evidence base) | the same ops as the after drive; the focus as above | [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/c0cadd96c0536dc4ed15b1a631d5fe0f7dbd792d/import-wizard-initial-focus/drive.sh.txt) |
| Live drive 1 (agent) | `28a44bf88` | opening, Cancel and the confirm right; two misses: Tab from the focused popup stayed on it (X76), and after "I’ve quit it" and after Try again the focus was nowhere: the screen went to Checking/Importing and back before the root's task fired, so a key made of the step's name came back to its value and the task disarmed | — |
| Live drive 2, the retry (agent) | `f009dac06` | every row as the reference (the popup's Tab and Shift+Tab; `focusKey` counts screen changes) | [focus-record.txt](https://raw.githubusercontent.com/ccheever/exact2/28d74170d767434f4768e7589f7b164550e5b8ec/import-wizard-initial-focus/focus-record.txt), [compose.py](https://raw.githubusercontent.com/ccheever/exact2/bfe66cecd5bff9ff033637c28434baaddbb06493/import-wizard-initial-focus/compose.py.txt) |

Found, not in this task: on the Checking screen Escape closes the reference's wizard (Base UI's dismissal; Checking can
close), while the clone's Checking screen has no button that carries Escape, so Escape does nothing there (read from the
sources, not driven).

## Not done / not verified

- Focus ring under real keys: real-input step 1 (the screen was locked; the agent's window is never key).
- Base UI's focus trap (Tab past the last stop and Shift+Tab before the first wrap inside the reference's dialogs; in the
  clone they leave the wizard and the confirm, before this change as after): EXACT2-GAPS X53,
  [#282](https://github.com/ccheever/exact2/issues/282), approved, waits for the main fix (partial in main PR #327).
- The fallback after a successful removal is Add profile, which is where the reference's history-based fallback landed
  in the run; the reference's target depends on which popups opened before (the clone keeps no such history).

## Delivery

Draft PR [#409](https://github.com/ccheever/exact2/pull/409) into `feat(example)/t3-code`.

## Next action

Coordinator: review the draft PR; run real-input step 1 in the next batch.
