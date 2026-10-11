---
name: 20261011-dialog-trap-and-progress-value
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

# Tab stays inside every dialog; progress bars tell assistive tech their value

## Outcome

STATUS "Known differences" rows 7 and 8 (the user asked on 2026-10-11 to fix what can be fixed now).

| Id | Clone | Reference |
| --- | --- | --- |
| DT-1 | In the import wizard (Settings › Integrations › Browser profiles › Add profile › Brave, "Quit Brave to import"), Shift+Tab from Cancel leaves the dialog for the Settings page behind it ("Add host", then "Built-in default…") (`realinput-1010h`, [image](https://raw.githubusercontent.com/ccheever/exact2/7102c03c8286911606342d14bda00cec33d3883d/realinput-1010h/IW-c-1to4-brave-dialog-rings.png)). Tab past the last stop of the wizard and of the profile removal confirm leaves them too ([import-wizard-initial-focus](closed/20261010-import-wizard-initial-focus.md), Not done). Exact keeps no Tab inside `aria-modal` on macOS (X53, [#282](https://github.com/ccheever/exact2/issues/282)); most of the clone's dialogs carry their own `key` traps since [dialog-shortcut-focus](closed/20261008-dialog-shortcut-focus.md), these do not | Base UI's focus trap: Tab past the last stop goes to the first, Shift+Tab before the first goes to the last, in every Dialog and AlertDialog |
| PV-1 | Progress bars (the Antigravity runtime download, and any other `role="progressbar"` box) are drawn boxes with the percentage only in `aria-description`; on macOS a drawn box's `progressbar` role is not exposed either (X55, #278) and Contract carries no value (X49, main `issues/20261009-aria-range-accessibility-values.md`, open) | `<progress>` / Radix progress with its value: VoiceOver reads "Antigravity download, 30 percent" |

## Steps

1. DT-1: list every dialog, alert dialog and modal popup of the clone (grep `aria-modal`, `role="dialog"`,
   `role="alertdialog"`) and check Tab past the last stop and Shift+Tab before the first in each, in agent mode (`key Tab`,
   `key shift+Tab`, then `focused`). Give each one that leaves its dialog the shared trap the others use (the wizard's own
   `popupKeys` already handles the popup itself holding the focus, X79). Keep the reference's order of stops.
2. PV-1: with no framework change, expose the value as close to the reference as Contract allows on macOS, and check it
   with `tree --ax` (and an out-of-process AX read, as RG-4 did, if the tree is not enough): for example an indeterminate
   `progress` element (which macOS does expose) named with the percentage, or the percentage in the accessible name. Keep
   the drawn bar as it looks now. If nothing reaches assistive tech, record the measurement and leave the row open
   (X49 stays the framework gap).
3. Tests; one agent drive (the wizard with the lane's fixture browser home, the profile removal confirm, the download bar
   with the fixture install); before / after images (focused element read-outs for DT-1, the AX read for PV-1).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DT-1 | every dialog listed with its result before and after; tests; agent drive | focused-element read-outs |
| PV-1 | `tree --ax` / AX read before and after | text before/after |
| The traps and VoiceOver under real keys | real-input steps for the final session | — |

## Next action

Start now.
