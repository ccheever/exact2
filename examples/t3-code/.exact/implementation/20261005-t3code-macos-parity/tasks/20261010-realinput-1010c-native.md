---
name: 20261010-realinput-1010c-native
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

# Failures of the attended real-input session of 2026-10-10 (realinput-1010c): native and app-level rows

## Outcome

The attended session (`realinput-1010c`, normal launches of lane copies, real keys and pointer, the user present for
sign-ins, notifications and the Korean input source) passed most rows. These steps failed under real input, although
their agent-mode and unit checks passed. This task fixes each as the reference does and writes exact real-input steps
for the next session. (#349's own failures are in its fix round, not here.)

## Findings

| Id | From | Reference (T3 Code `1e2ecbd975`) | Clone under real input | Evidence |
| --- | --- | --- | --- | --- |
| RC-1 | #373 RI-2 | A click on the permission helper's "T3 Code" row reveals the app in Finder and Finder comes to the front; the helper hides. | Finder opens a window with the bundle selected but stays behind System Settings; the helper stays docked; T3 Code does not come forward either (2 of 2, base build `ec6f9bf1c`). #373 added the reference's activation order; it is not enough. | [B2](https://raw.githubusercontent.com/ccheever/exact2/35cd1bf5e87832d50bc53de69f0a225ce3852ede/realinput-1010c/B2-ri2-helper-row-click.png) |
| RC-7 | every lane copy | (check) The reference raises no Documents-folder permission prompt at launch. | Lanes 5-9 and 11-13 raised "would like to access files in your Documents folder" at launch, with HOME set to the lane home, so something resolves the real `~/Documents` (the account's home, not `HOME`). Find the access; remove it if the reference does not make it. | [E-observation](https://raw.githubusercontent.com/ccheever/exact2/ce8a494e3bf39535de879b08948f805908ee14db/realinput-1010c/E-observation-documents-prompt.png) |
| RC-9 | B (check) | (check) T3 Code's Quit shortcut behavior by default (Settings › General › quit behavior). | One ⌘Q does not quit: the clone's default is Hold; two quick presses quit. Compare with the reference's default; change only if it differs. | [B1 ⌘Q](https://raw.githubusercontent.com/ccheever/exact2/ddf1b0150989510e1372d7459664a4066df89f42/realinput-1010c/B1-cmd-q-menu.png) |

## Scope and exclusions

Included: RC-1, RC-7 and RC-9 (RC-7 and RC-9 start as checks). The UI rows are in
[realinput-1010c-fixes](20261010-realinput-1010c-fixes.md).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RC-1 | an AppKit test of the activation path; exact real-input steps for the next session (stays open until it runs) | text (front app read-back) |
| RC-7 | find the access (log file opens or `fs_usage` on a lane launch); compare with the reference lane; remove it if the reference does not make it | text |
| RC-9 | reference comparison of the quit-behavior default; change only if it differs | text |

## Next action

Start now. Real-input checks join the next attended session.
