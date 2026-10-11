---
name: 20261005-x28-notification-actions-badges
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-policy
blocks: [20261005-client-activity-reporting]
upstream_url: https://github.com/ccheever/exact2/issues/114
reproduced_on: null
---

# X28: Notification click → app action, Dock badge, window-focus fact (DEFERRED refuses actions and badges)

Moved to main `issues/20261009-notification-default-click-design.md` (2026-10-09); tracked there.

## Summary

T3 Code tells a person when a thread finishes or needs attention: an OS notification that, when clicked, focuses the app and opens that thread, a Dock badge that counts pending notifications, and a decision based on whether the window has focus. exact2's framework refuses notification actions and badges by a DEFERRED rule (`rules/DEFERRED.md:367-369`) and gave the page no window-focus fact. The clone does the click and the badge in its own Swift module; the focus fact came from main #219.

## Why it arose

### The T3 Code behavior

Reference: `apps/web/src/components/ThreadNotificationCoordinator.tsx`, `apps/web/src/threadNotifications.ts`, `apps/desktop/src/ipc/methods/notificationBadge.ts`.

- **Settings.** `notificationMode` is `off`, `notifications`, `sound` or `notifications-and-sound` (`threadNotifications.ts:7–13`); `inAppNotificationsEnabled` is separate.
- **Trigger.** For each thread whose sidebar status changes to completion or to a state that needs the person, the coordinator builds a title ("Thread completed", "Approval needed", "Usage limit reached", "Thread failed", "Input needed"); the body is the thread title (`ThreadNotificationCoordinator.tsx:140–175`).
- **While the window is visible and focused** and the thread is not the open one: an in-app toast with an "Open thread" action instead of an OS notification (lines 160–195).
- **While the window is not focused** and permission is `granted`: an OS notification, `silent: true`, tagged `<environmentId>:<threadId>` (lines 196–216). **Click:** close it, focus the window, open that thread (lines 208–216).
- **Dock badge.** The count of pending notifications is sent to the desktop process (`setNotificationBadge`, `threadNotifications.ts:23–60`), which sets `app.setBadgeCount(count)` (`notificationBadge.ts:22–45`); it clears on window focus, before quit, and when the page asks.
- Tests: `ThreadNotificationCoordinator.test.tsx`, `ThreadNotificationCoordinator.badge.test.tsx`, `apps/desktop/src/ipc/notificationBadge.test.ts`.

### Where the clone hits it

`20261005-client-activity-reporting` needed focus and visibility for its activity reports; the thread notifications needed the focus fact, the click and the badge.

## Clone workaround

- `modules/apple/T3Notifications.swift` (header names the reference files): a `UNUserNotificationCenter` delegate posts the notification (tagged per thread), plays the two sounds with `NSSound`, sets `NSApp.dockTile.badgeLabel` to the pending count and clears it on `didBecomeActive` and `didBecomeKey`, and on a click activates the app and tells the window which thread to open (`open(threadId:)`, `didReceive`). Under the agent the app never asks for notification permission.
- Since main #219 (below) the focus is the page's `hasFocus`: `shell-notify.ts` decides between the in-app toast and the system notification on `page.hasFocus`, and `T3Notifications.swift` posts whenever the page asks.
- #224's decision (2026-10-08): the Dock badge and its clearing, and the permission reading, are a permanent declared difference in `T3Notifications.swift`; the click that opens the finished thread is the part main still tracks.
- The agent cannot see or click a notification, so permission, banner, sound, click-to-open and the badge are verified only in attended sessions (`AGENT-HANDOFF.md` checklist item 7).

## Evidence and history

- Filed 2026-10-06 as [#114](https://github.com/ccheever/exact2/issues/114). A clone lane drive (`target/t3-ui-parity/lanes/r12-threads/drive-receipts.ndjson`) showed the page facts `online`, `root-font-size`, `visibility-state` and `can-share`, no focus fact.
- 2026-10-07 ([adopt-main-fixes-r5](../../tasks/closed/20261007-adopt-main-fixes-r5.md), main `261dd4e10`): main #219 (`6af680b0e`) closed #114 with `exactPage().hasFocus` (`document.hasFocus()` on every host; the agent sets it with `prefer has-focus`). Click actions and the badge continued as [#224](https://github.com/ccheever/exact2/issues/224). Adopted:
  - **Thread notifications.** `app.contract` declares `resource page = exactPage() as shape Page` and hands `page.hasFocus` and `page.visibilityState` to `shellView`; `T3Notifications.swift` no longer reads `NSApp.isActive`.
  - **Client activity reports.** `visible` and `focused` are the page's `visibilityState` and `hasFocus`, handed to the reporter through the module's `activityFacts` op (`reportWindowFacts`); `T3ActivityReporter.swift` keeps only the pointer, key and wheel interactions.
  - **Other focus listeners:** the workspace card asks `vcs.refreshStatus` when the window regains the focus (`refreshVcsOnFocus`), and SnapShot settings read the permissions again on focus.
  - Live (agent, lane server through a trace proxy): before, every report said `focused:false`; after, `focused:true`; `prefer has-focus false` gives `focused:false`, `shell.notifications` "… (window inactive)"; focus back gives `vcs.refreshStatus` and a report with `focused:true`.
  - Not built then (the clone had no counterpart): the reference's toast timers that run only while focused, DiffPanel's refresh on focus, the composer's refocus on window focus, SnapShotCoordinator's drain on focus. They can be built on `page.hasFocus`.
