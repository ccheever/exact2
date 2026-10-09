---
name: 20261009-notifications-all-environments
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

# Thread notifications watch every connected environment

## Outcome

A completion, approval, failure, usage limit or input request in any connected environment notifies, as in the
reference (the in-app toast with "Open thread", the system notification, the sound). Today only the focused
environment's threads notify.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)), from source. Reference: T3 Code
`1e2ecbd975`. Clone: `c603c22d6`.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PG-10 | `ThreadNotificationCoordinator` mounts `EnvironmentNotifications` for every connected environment, so any environment's turn transition notifies. | `threadNotifications` diffs only `client.shell` (the focused environment's `/api/orchestration/shell`) and resets when the focused environment changes (`shell-notify.ts:97-106`, `shell.ts:262`). Threads in other connected environments never notify. | Connect two environments, focus A, let a turn complete in B with notifications or in-app toasts on. | `examples/t3-code/shell-notify.ts` (source comparison) |

## Scope and exclusions

Included: watching every connected environment's shell for the reference's transitions, with each notification opening
the right environment's thread.

Excluded:
- The Dock badge and notification action buttons: [#224](https://github.com/ccheever/exact2/issues/224) (X28), the badge a
  permanent declared difference, the click waiting for a ruling.
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`): `components/ThreadNotificationCoordinator.tsx:27-91`.

Clone (`examples/t3-code`): `shell-notify.ts:97-106`, `shell.ts:262`; the connected environments (the fleet entries that
Settings' scope already reads: `settings-b-fleet.ts`, `connections.ts`). Keep the focused environment's behaviour
unchanged.

Lane: two lane servers (as pr-links-previews-and-routing used), each with a fixture thread whose turn can be completed
without a signed-in provider (a projection change), or a mock provider. Send no message to a real provider.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PG-10 | Focus A; complete a turn in B: the in-app toast "Thread completed" with "Open thread" shows, and "Open thread" opens B's thread. Bun test: transitions in a non-focused environment produce notifications. | `pg10-other-environment-toast.png` | agent |
| PG-10 | With the window unfocused, B's completion posts a system notification with the sound setting. | `pg10-system-notification.png` | needs_real_input (a real unfocused window and Notification Center) |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build the two-server lane. Build and unit-test. Then do one batched live
drive at the end, and the system-notification row when the screen is unlocked. Close every row in this PR, or record the
blocker of a row that cannot pass.
