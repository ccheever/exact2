---
name: 20261010-realinput-1010c-fixes
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

# Failures of the attended real-input session of 2026-10-10 (realinput-1010c)

## Outcome

The attended session (`realinput-1010c`, normal launches of lane copies, real keys and pointer, the user present for
sign-ins, notifications and the Korean input source) passed most rows. These steps failed under real input, although
their agent-mode and unit checks passed. This task fixes each as the reference does and writes exact real-input steps
for the next session. (#349's own failures are in its fix round, not here.)

## Findings

| Id | From | Reference (T3 Code `1e2ecbd975`) | Clone under real input | Evidence |
| --- | --- | --- | --- | --- |
| RC-1 | #373 RI-2 | A click on the permission helper's "T3 Code" row reveals the app in Finder and Finder comes to the front; the helper hides. | Finder opens a window with the bundle selected but stays behind System Settings; the helper stays docked; T3 Code does not come forward either (2 of 2, base build `ec6f9bf1c`). #373 added the reference's activation order; it is not enough. | [B2](https://raw.githubusercontent.com/ccheever/exact2/35cd1bf5e87832d50bc53de69f0a225ce3852ede/realinput-1010c/B2-ri2-helper-row-click.png) |
| RC-2 | #374 CO-7 | After a Shift+click removes a model, the picker trigger names the remaining model. | The accessible label is right ("Claude Sonnet 5.5"), but the painted trigger text stays stale ("Claude Fable 5.1, C…") in a box sized for the new label (2 of 2). | [C](https://raw.githubusercontent.com/ccheever/exact2/4227ea6597bae1028438390c0516437d80da45c3/realinput-1010c/C-co7-shift-pick.png) |
| RC-3 | #378 FW-3 step 3 | With the pointer moved into Search authors: Escape closes Author, ↓ moves nothing, a second Escape closes Filters. | The last Escape does not close Filters; only a click outside does (2 of 2). | [G](https://raw.githubusercontent.com/ccheever/exact2/e8fabfeed724dce27c432773cd615219967a8fea/realinput-1010c/G-fw3-fw4-filters.png) |
| RC-4 | #364 steps 1-3 | The skill chip's popover closes on Escape; in Settings' prompt sample, Escape closes only the popover; the chip's accessible element is an enabled button. | After clicking the heading and then the chip, Escape leaves the popover open (2 of 2); in Settings, Escape closes the popover and Settings (2 of 2); the chip's "Skill Frontend Design. Show details" element reports itself disabled. | [H](https://raw.githubusercontent.com/ccheever/exact2/5df0a1d5d26d8b1de7ce55614125bfc0f66a6b7a/realinput-1010c/H-skill-chip.png) |
| RC-5 | #375 PG-3 | Moving the pointer from the (i) up into the unpriced popover keeps it open. | A ~4 pt gap between the (i) and the popup closes it on the way. | [I](https://raw.githubusercontent.com/ccheever/exact2/a0aabaf843cfa53420383227b9844aa36dfb324d/realinput-1010c/I-pg3-pg4.png) |
| RC-6 | #354 step 2 | In the Add profile menu, ↓ walks the rows from Blank profile to Chrome and the other browsers. | ↓ opens on Blank profile but never reaches Chrome (3 tries); the row ⋮ menu and the launcher chevron work. | [K](https://raw.githubusercontent.com/ccheever/exact2/b5e68e2a741418544b1608af7933ba5677f55e30/realinput-1010c/K-profiles.png) |
| RC-7 | every lane copy | (check) The reference raises no Documents-folder permission prompt at launch. | Lanes 5-9 and 11-13 raised "would like to access files in your Documents folder" at launch, with HOME set to the lane home, so something resolves the real `~/Documents` (the account's home, not `HOME`). Find the access; remove it if the reference does not make it. | [E-observation](https://raw.githubusercontent.com/ccheever/exact2/ce8a494e3bf39535de879b08948f805908ee14db/realinput-1010c/E-observation-documents-prompt.png) |
| RC-8 | #360 (check) | Diff › Changes shows the thread's project (the lane's `work` repo). | On the #360 bundle the Changes view showed the enclosing exact2 checkout ("feat(example)/t3-code vs origin/main", +576k) for a lane project; and a click on the composer did not focus it. Check whether this is the lane's project path (inside the exact2 tree) or a clone bug; fix if the clone resolves the wrong repository. | [D](https://raw.githubusercontent.com/ccheever/exact2/d10b10619a6d71683e6eea6ccd6e2bbf53f3a604/realinput-1010c/D-pa12-cmd-return.png) |
| RC-9 | B (check) | (check) T3 Code's Quit shortcut behavior by default (Settings › General › quit behavior). | One ⌘Q does not quit: the clone's default is Hold; two quick presses quit. Compare with the reference's default; change only if it differs. | [B1 ⌘Q](https://raw.githubusercontent.com/ccheever/exact2/ddf1b0150989510e1372d7459664a4066df89f42/realinput-1010c/B1-cmd-q-menu.png) |

## Scope and exclusions

Included: the nine rows (RC-7, RC-8 and RC-9 start as checks). Excluded: #349's failures (its own fix round); the
reaction-pill tooltip of #261 (waits for main #327, #322).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RC-1..RC-6 | a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |
| RC-7 | find the access (log file opens or `fs_usage` on a lane launch); compare with the reference lane | text |
| RC-8, RC-9 | reference comparison; fix only a real difference | text |

## Next action

Start now. Real-input checks join the next attended session.
