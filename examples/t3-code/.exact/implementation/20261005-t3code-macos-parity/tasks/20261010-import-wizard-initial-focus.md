---
name: 20261010-import-wizard-initial-focus
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

# The browser import wizard's initial focus

## Outcome

When the browser import wizard opens on Configure, T3 Code (`1e2ecbd975`) focuses its first tile ("Personal / 5
cookies"). `BrowserImportWizard.tsx:145-146` is a Base UI `Dialog` / `DialogPopup` with no `initialFocus` and no
`autoFocus`, so the popup's first tabbable element takes the focus, and the close X comes after the step
(`ui/dialog.tsx` `DialogPopup`). Base UI focuses only when the dialog opens, not when a step changes. The clone focuses
Import (`browser-profiles.contract`, `BiButton(buttonId="browser-import-run", … first=true)`, `autofocus=first`). Its
other steps each autofocus a button ("I’ve quit it", Cancel on Full Disk Access, Done, Close), and `autofocus` applies
again at every step change. Found by [realinput-1010e-followups](20261010-realinput-1010e-followups.md) RE-4 (#406).

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

## Next action

Start now.
