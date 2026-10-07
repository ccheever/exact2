---
name: 20261008-dialog-shortcut-focus
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

# Dialog keyboard focus reaches invisible shortcut buttons

## Outcome

Keyboard navigation through a modal must not focus the app's zero-size shortcut dispatch
buttons. Preserve working keyboard shortcuts, Escape cancellation and focus return.

## Evidence and scope

Reported during PR #238's attended session on 2026-10-07/08:
[provider sign-in task](closed/20261005-provider-sign-in-and-install.md#attempts-and-evidence).
Tab from a dialog's last button reached `settings-shortcuts.contract`'s `keyboard-dispatch`
buttons. The original report describes this as app-wide and pre-existing. It has not been
reproduced again on `29dbc5dbf`; the root cause and framework/app ownership remain unconfirmed.
The provider dialog fix that makes Settings inert is already implemented and is distinct
from this remaining focus stop. No implementer is assigned yet.

Start with `settings-shortcuts.contract`, `keyboard-dispatch.ts`, `AppConfirm` in
`shell-panels.contract`, and the modal wiring in `app-settings.contract` / `app.contract`.
Keep the fix limited to focus eligibility and modal behavior. If a minimal reproduction
proves a framework limitation, record that evidence before classifying it as one.

## Reproduction and acceptance

1. Build the current macOS app with an isolated lane home. Open Settings > Providers and
   a fixture-backed Sign out or Remove confirmation; do not sign out a retained real account.
2. Use real Tab and Shift+Tab through the controls, including past the final button.
   Record focused elements and the accessibility tree. Repeat with another app dialog.
3. Verify no zero-size dispatcher or inert Settings control takes focus. Check that focus
   follows the modal's intended visible controls in both directions.
4. Escape must dismiss without sending the confirm RPC and return focus to the trigger.
   Return on Confirm must send exactly one intended operation.
5. With the dialog closed, verify representative keyboard shortcuts still work, including
   a configured shortcut and a shortcut while a text field is focused.

## Next action

Reproduce on the latest parent branch, then implement and add a focused regression test
where the failure occurs. Record the revision and real-keyboard before/after evidence.
If no longer reproducible, record the fixing revision or concrete current evidence before
closing; do not infer a fix from successful compilation.
