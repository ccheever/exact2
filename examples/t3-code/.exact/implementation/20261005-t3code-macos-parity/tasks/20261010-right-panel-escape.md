---
name: 20261010-right-panel-escape
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

# Escape and the right panel

## Outcome

The clone's "Toggle right panel" button declares Escape (`r4-surfaces.contract`, `testId="panel-toggle-right"`,
`aria-keyshortcuts` … `"Escape"`) whenever a panel is open and no editor, URL field, device setup or annotate pick has
the key. So Escape closes the right panel (Diff, Files, Browser, …). This has been there since the clone's first commit
(`b0a14280a`), and no record says why. T3 Code (`1e2ecbd975`) binds no Escape to the right panel: its defaults are
`rightPanel.toggle` = mod+alt+b and `rightPanel.close` = mod+w when the terminal is not focused
(`packages/shared/src/keybindings.ts:26,31`). Its components handle Escape only inside their own fields (the Files
search, a device tab's rename, the PR detail panel's own controls, the PR page's "Let panel shortcuts consume Escape
before page navigation"). Found in the review of #399 (realinput-1010c-fixes, RC-4).

## Steps

1. Check the live reference first (`target/t3-audit/ref-app.sh`, CDP): with each of Diff, Files, Browser and the
   Pull Requests page's panel open and the focus in the page (not in a field), press Escape. Record whether the panel
   closes. Also check with the focus on the panel's own toggle button.
2. If the reference never closes the panel on Escape, remove the clone's Escape from the toggle. Check what relied on it:
   tests that press Escape to close the panel, the Browser annotate pick (`panel.browser.capture.pickActive`), the file
   editor menu (`0fc543028`, "dismiss the file editor menu before the panel"), and the chip details' Escape order
   (`ChipPopoverEscape`, #399). If the reference closes it in some case, match that case only.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Reference behaviour | CDP drive of the live reference, each panel kind | text record |
| Clone matches | unit test of the toggle's keys; one agent drive: a panel open, Escape, the panel stays (or matches the reference's case) | before / after image |

## Next action

Start now.
