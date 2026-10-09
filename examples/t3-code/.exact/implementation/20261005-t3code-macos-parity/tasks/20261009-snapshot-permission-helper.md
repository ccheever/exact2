---
name: 20261009-snapshot-permission-helper
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

# The macOS permission helper window beside System Settings

## Outcome

When the app opens System Settings › Privacy for Screen Recording or Accessibility, it shows the reference's small helper
panel docked under the System Settings window. The panel says "Set up <permission>", shows "↑ Drag T3 Code into the list
above" with a T3 Code button you can drag (a click reveals the app in Finder), and has a close button. It follows the
Settings window and closes when the grant is detected.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)), from source (the screen was
locked). Reference: T3 Code `1e2ecbd975`. Clone: `c603c22d6`.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PG-9 | A packaged macOS build: after it opens Privacy › Screen Recording or Accessibility (SnapShot setup, `requestPermissions`, `openSystemSettings`), a "Set up <permission>" panel docks under the System Settings window: "↑ Drag T3 Code into the list above", a draggable T3 Code button (a click reveals it in Finder) and a close button. It tracks the Settings window and closes when the grant is detected. | Only opens the `x-apple.systempreferences` URL: no helper panel, no drag source, no grant polling (`modules/apple/T3SnapShot.swift:274-300`). | Settings › SnapShots › enable without Screen Recording granted; watch beside System Settings. | `target/t3-ref/src-1e2ecbd975/apps/desktop/src/permissions/MacPermissionHelper.ts` (source comparison) |

## Scope and exclusions

Included: the helper panel, its drag source and Finder reveal, docking to the System Settings window, and closing on the
grant, for Screen Recording and Accessibility.

Excluded:
- Changing the user's real privacy grants. A row that needs a grant or a revoke is attended: the user does it in person.
- Framework code. The auditor's reading: an AppKit `NSPanel` in the clone's native module needs no framework support.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/desktop/src`): `permissions/MacPermissionHelper.ts`,
`permissions/MacSettingsWindow.ts`, `permissions/MacPermissions.ts:21-40`; `snapShot/DesktopSnapShot.ts:1305-1325, 1399`;
`ipc/methods/window.ts:314-330`.

Clone (`examples/t3-code`): `modules/apple/T3SnapShot.swift:274-300` (opens the System Settings URL). Keep the
permission messages, which already match ("… then restart T3 Code.").

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state). The agent's
`screenshot … window` captures only the app window, so this task's pairs are real screen captures taken while the
screen is unlocked.

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PG-9 | Enable SnapShots without Screen Recording: System Settings opens on Screen Recording and the helper docks under it with the drag button and close. A click on the button reveals the app in Finder. The close button closes it. | `pg9-helper-docked.png` | needs_real_input (System Settings and a real screen) |
| PG-9 | Grant detection: after the user grants the permission, the helper closes by itself. Same for Accessibility. | `pg9-helper-closes.png` | needs_real_input, attended (the user grants and later revokes in person) |
| PG-9 | Unit or XCTest: the panel's placement follows a given Settings window frame. | text: test output | XCTest |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build the panel with XCTests first. Run the real-input rows in one session
while the screen is unlocked, with the user present for the grant row. Close every row in this PR, or record the blocker
of a row that cannot pass.
