---
name: 20261009-notifications-all-environments
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-notifications-all-environments
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

## Results

Built on `feat(example)/t3-code-notifications-all-environments`
(`shell-notify.ts`, `client-ops-threads.ts`, `shell.ts`; tests in `shell.test.ts`).

- `threadNotifications` is ThreadNotificationCoordinator over every environment: `notifyEnvironments` lists the focused
  environment (T3Client's shell, live while the client is ready, as before) and every background environment the fleet
  keeps (`settings-b-fleet.ts`; live once its transport is connected and synchronized for its generation). Each environment
  has its own EnvironmentNotifications memory, cleared while its shell is not live and dropped when it leaves the list; the
  first live shell only records. A not-ready focus no longer holds back the other environments.
- A toast's "Open thread" and a posted notification name the thread with its environment, `fleet:<environment>:<thread>`
  (the reference's `/$environmentId/$threadId`); the notification tag is `<environment>:<thread>` as before.
  `select-thread` opens such an id in place when its environment is the focus, and focuses the environment otherwise
  (`focusFleetThread`), so a toast or notification opens the right thread even after the focus moved.
- Only the open thread of the focused environment is quiet (the reference's `activeEnvironmentId !== environmentId ||
  activeThreadId !== thread.id`); a background thread with the same id as the open one notifies.
- Not changed (excluded above, #224): the Dock badge and the notification center's pending list. The reference also
  closes the delivered notifications of an environment that leaves its catalog; that bookkeeping is the badge's
  (`T3Notifications.swift` clears every pending notification when the window gains focus).

| Id | Row | Result | Proof |
| --- | --- | --- | --- |
| PG-10 | Focus A; complete a turn in B: "Thread completed" with "Open thread"; Open thread opens B's thread | pass (agent drive) | [pg10-other-environment-toast.png](https://raw.githubusercontent.com/ccheever/exact2/263e6c37276c861afb70c0dddaef1815dc9d0d15/notifications-all-environments/pg10-other-environment-toast.png), [pg10-open-thread.png](https://raw.githubusercontent.com/ccheever/exact2/71469ff91caafd24b59e7af835b77e85bc31be7e/notifications-all-environments/pg10-open-thread.png) |
| PG-10 | Bun test: transitions in a non-focused environment produce notifications | pass | `shell.test.ts` "every connected environment (ThreadNotificationCoordinator)" (6 tests); the probe below |
| PG-10 | Unfocused window: B's completion posts a system notification with the sound setting | open: needs real input | Bun test "unfocused: the system notification is tagged with B and opens B's thread"; batch steps below |

Text before/after (a probe calling `threadNotifications` with A focused and a background fleet entry B whose thread
completes; `target/notify-lane/pg10-probe.ts`, not committed):

```
== before (feat(example)/t3-code 6e2040c58) ==
focused window, B completes: []
  sounds: []
unfocused window, B completes again: notifyPost []
== after (branch) ==
focused window, B completes: [{"title":"Thread completed","description":"Build the lane B report","action":{"label":"Open thread","op":"select-thread","id":"fleet:env-b:b1"}}]
  sounds: ["completion"]
unfocused window, B completes again: notifyPost [{"title":"Thread completed","body":"Build the lane B report","tag":"env-b:b1","threadId":"fleet:env-b:b1"}]
```

Lane (2026-10-09, base port 16960, not committed): B is the reference app's backend (`ref-app.sh`, 16960) with its own
environment id and its Grok provider pointed at a lane stand-in CLI that runs the reference's own
`apps/server/scripts/acp-mock-agent.ts` behind a proxy holding each `session/prompt` until a trigger file exists (no
real provider; the lane's Codex and Claude paths point at missing binaries on both sides). The turn was sent from the
reference's UI; each clone drive (before: `t3-code-evidence-base`, after: this branch; same ops, 1280×840) paired B through
Settings › Connections › Add environment with a fresh `t3 pair` link, stayed focused on This machine, and the conductor
released B's held turn after the `-pre` screenshot. The before build showed B's row turning Done with no toast; the after
build showed "Thread completed · Build the lane B report · Open thread", and Open thread focused B with that thread open.
The reference showed the same toast for the same completion and opened the same thread.

## Real-input batch steps

Row "Unfocused window: system notification" (needs Notification Center and a real unfocused window):

1. Start the reference as B: `ROOT/target/t3-audit/ref-app.sh notifications-all-environments 16960` (`<lane>` below is
   `ROOT/target/t3-audit/lanes/notifications-all-environments`: its `bin/` holds the Grok stand-in and the held-turn proxy,
   and `<lane>/ref-t3-home` has its own environment id).
2. From this branch's worktree, launch the clone normally on the lane home:
   `T3_LOCAL_HOME=<lane>/clone-t3-home T3_LOCAL_PORT=16962 EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs
   t3-code-macos --bundle --run`. In Settings › General set Thread notifications to "Notifications and sound" and allow
   notifications when macOS asks.
3. Settings › Connections › Add environment: paste a fresh link from
   `ROOT/target/t3-audit/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64/t3 pair --base-dir <lane>/ref-t3-home`,
   Connect. Close Settings; keep This machine focused.
4. In the reference window, open "Build the lane B report" and send "batch turn" (the proxy holds it), then open another
   thread in the reference.
5. Click another app (Finder) so the clone's window is not focused. Release the turn:
   `touch <lane>/release-turn`.
6. Expected: within a few seconds a macOS notification "Thread completed" / "Build the lane B report" appears (Notification
   Center), the completion sound plays once, and no in-app toast is shown. Click the notification: the clone comes to the
   front, focuses B and opens "Build the lane B report".
7. Stop the reference with `ref-stop.sh notifications-all-environments` and quit the clone.

## Next action

Coordinator: review the draft PR; run the real-input batch steps above for the system-notification row.
