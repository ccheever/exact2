---
name: 20261005-x28-notification-actions-badges
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-policy
blocks: [20261005-client-activity-reporting]
upstream_url: https://github.com/ccheever/exact2/issues/114
reproduced_on: null
---

# X28: Notification click → app action, Dock badge, window-focus fact (DEFERRED refuses actions and badges)

## Summary

T3 Code tells a person when a thread finishes or needs attention: an OS notification that, when clicked, focuses the app and opens that thread, a Dock badge that counts pending notifications, and a decision based on whether the window has focus. exact2's framework refuses notification actions and badges by a DEFERRED rule and gives the page no window-focus fact. The clone does all three in its own Swift module and the result matches the reference, so this issue is mainly a policy question for Charlie: keep the module path as the answer (close by decision) or add framework support.

## Why this issue arose

### The T3 Code behavior

Reference: `apps/web/src/components/ThreadNotificationCoordinator.tsx`, `apps/web/src/threadNotifications.ts`, `apps/desktop/src/ipc/methods/notificationBadge.ts`.

- **Settings.** `notificationMode` is `off`, `notifications` ("Notifications only"), `sound` ("Sound only") or `notifications-and-sound` ("Notifications with sound") (`threadNotifications.ts:7–13`); `inAppNotificationsEnabled` is separate.
- **Trigger.** For each thread whose sidebar status changes to completion or to a state that needs the person, the coordinator builds a title: "Thread completed", "Approval needed", "Usage limit reached", "Thread failed" or "Input needed"; the body is the thread title (`ThreadNotificationCoordinator.tsx:140–175`).
- **While the window is visible and focused** and the thread is not the open one: an in-app toast with the same title and an "Open thread" action instead of an OS notification (lines 160–195). Sounds play for `sound` modes from two bundled files.
- **While the window is not focused** and permission is `granted`: an OS notification, `silent: true` (the app plays its own sound), tagged `<environmentId>:<threadId>` so a newer one for the same thread replaces the older (lines 196–216). **Click:** close it, focus the window, open that thread (lines 208–216). A browser that rejects desktop presentation is ignored (try/catch).
- **Dock badge.** The count of pending notifications is sent to the desktop process (`setNotificationBadge`, `threadNotifications.ts:23–60`); the desktop sets `app.setBadgeCount(count)` on macOS and Linux (`notificationBadge.ts:22–45`). A focused window forces 0. The badge clears on window focus, before quit, and when the page asks (`onNotificationBadgeClear`); the page also clears on `focus` (coordinator lines 55–70). Pending notifications of an environment that disappears are closed (lines 44–53).
- Tests: `ThreadNotificationCoordinator.test.tsx`, `ThreadNotificationCoordinator.badge.test.tsx`, `apps/desktop/src/ipc/notificationBadge.test.ts`.
- Permission: an OS notification is posted only when `Notification.permission` is `granted` (coordinator lines 196–200). The Settings page asks with `Notification.requestPermission()` (`components/settings/NotificationSettings.tsx:50`). Choosing a notification mode in Settings asks first; if the answer is not `granted` the mode is not saved and the page says "Allow notifications in your browser or system settings, then choose this option again. Sound only is still available." Without permission only the in-app toast and the sound remain.

### What exact2 does today

From `EXACT2-GAPS.md` (written by earlier sessions from framework source at `c1522fdac`, checked against `main` `d2cb661eb`; citations as given there): table row X28 "Notification click → app action, Dock badge, window-focus fact … policy (DEFERRED refuses actions/badges)"; and "`showNotification` exists (`4754c6d9e`), but DEFERRED refuses notification actions and badges (`rules/DEFERRED.md:367-369`); no focus fact (`runner/src/page.rs:21-36`)."

Observed in a clone lane drive (`target/t3-ui-parity/lanes/r12-threads/drive-receipts.ndjson`): the page facts the agent reports are `online`, `root-font-size`, `visibility-state` and `can-share`; there is no `focused` fact. This agrees with the quoted line.

Bundled library (`20261005-platforms-v3`): notifications, badges and focus facts are **not covered: unknown**. I did not open `rules/DEFERRED.md`; the rule text itself is to confirm at `issue-open`.

### Where the clone hits it

`modules/apple/T3Notifications.swift` (185 lines; header names the reference files): a `UNUserNotificationCenter` delegate posts the notification (tagged per thread, banner and list when the app is not active), plays the two sounds with `NSSound`, sets `NSApp.dockTile.badgeLabel` to the pending count and clears it on `didBecomeActive` and `didBecomeKey`, and on a click activates the app and tells the window which thread to open (`open(threadId:)`, `didReceive`). The focus is `NSApp.isActive` and a visible key window (`active`, line 44). TypeScript has no focus event, so `shell-notify.ts` reads it whenever the shell asks (`notifyStatus` op) and posts through `notifyPost`. Under the agent the app never asks for notification permission and reports the center's state, labeled as an agent run.

What differs from the reference: nothing is known to differ for a person. What is harder: the agent cannot see a notification or click one, so permission, banner, sound, click-to-open and the badge are verified only in attended sessions (`AGENT-HANDOFF.md` checklist item 7; "os-grant" kind). The focus read is a poll tied to the shell's own refresh, not an event.

## Why it must be resolved

Parity goal: thread notifications are a core desktop behavior and the clone already reproduces them. What remains is a policy decision and a verification gap, not a user-visible difference:

- The framework's DEFERRED rule says actions and badges are refused. The clone's module bypasses nothing (it is app code), but it means every exact2 app that wants notification clicks or a badge must write Swift, and a pure-Contract app cannot.
- A window-focus fact is useful beyond notifications: `20261005-client-activity-reporting` needs focus, visibility and occlusion for its activity reports and reads them in Swift for the same reason.
- Decision needed from Charlie: (a) the module path is the sanctioned design, so close this issue; (b) lift the DEFERRED refusal for notification actions, a badge count and a focus fact; (c) lift only the focus fact (a read-only page fact, the least contentious part). The plan does not decide this.
- Cost of keeping the module: 185 lines of Swift, attended-only verification of the click and badge paths, and a poll for focus.

## Requested support

Stated the web way: the Notifications API already has `notification.onclick`, `Notification.permission` and `Notification.requestPermission()`; the Badging API has `navigator.setAppBadge(count)` and `clearAppBadge()`; page focus is `document.hasFocus()` and the `focus`/`blur` window events. On the macOS host first:

- **A. Web-standard surface.** `Notification` with `onclick` delivered to the page (or a Contract event), `navigator.setAppBadge` mapped to the Dock tile, and `document.hasFocus()`/`visibilitychange`/`focus` as page facts (also in agent `state`). Needs a DEFERRED waiver.
- **B. Focus fact only.** Add the read-only focus fact; leave notifications and badges to modules (decision (c)).
- **C. No framework change.** Close by decision; the module stays. Document the module pattern.

## How to reproduce

To confirm on the pinned `main` at `issue-open`:

1. Minimal app that calls the framework's `showNotification` and has no module. Click the notification. Expected (reference): the app focuses and a Contract action runs with the tag. Actual (per `EXACT2-GAPS.md`): no action reaches the app.
2. Set a badge count. Expected: the Dock tile shows it. Actual: there is no API.
3. Read focus: `state` in the agent. Expected: a `focused` fact. Actual: `visibility-state` only.
4. Clone: lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; set notifications on, background the app, finish a fixture thread; expect a banner, a Dock badge "1", and a click that opens the thread (attended session; macOS permission prompt).

## Acceptance for the fix

Under option A or B: a minimal app shows the focus fact flip on activate and deactivate (agent `state`, and a real window switch in an attended session); under A also a click reaches the page, and the badge count appears on the Dock tile and clears. Under C: a recorded user decision. In all cases the clone's attended notification rows (banner, sound, click-to-open, badge cleared on focus) pass.

## App adoption after resolution

Under A or B: replace the `notifyStatus` poll with the focus fact in `shell-notify.ts` and `client-activity-reporting`; under A also delete the delegate, badge and click code from `T3Notifications.swift`. Under C: mark the module as the sanctioned path in `EXACT2-GAPS.md` and close. `issue-close` verifies the attended rows.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (confirm the DEFERRED text, reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval). The policy decision belongs to Charlie.

## Merged upstream; focus fact adopted (2026-10-07, adopt-main-fixes-r5)

Filed as [#114](https://github.com/ccheever/exact2/issues/114). Main #219 (`6af680b0e`) closed it with option B:
`exactPage().hasFocus` is `document.hasFocus()` on every host (macOS: the app is active and the session's window
is key), it is told again on window focus changes, and the agent sets it with `prefer has-focus`. Notification
click actions and the app badge continue as the policy issue [#224](https://github.com/ccheever/exact2/issues/224)
(refused by DEFERRED). In the feature branch since main `261dd4e10` ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)).

Adopted:
- **Thread notifications.** `app.contract` declares `resource page = exactPage() as shape Page` and hands
  `page.hasFocus` and `page.visibilityState` to `shellView`. `shell-notify.ts` decides between the in-app toast and
  the system notification on `page.hasFocus`, as ThreadNotificationCoordinator reads `document.hasFocus()`.
  `T3Notifications.swift` no longer reads `NSApp.isActive` (its `active` and the agent's "a visible window is
  focused" rule are gone) and posts whenever the page asks.
- **Client activity reports** (`client-activity-reporting`). `visible` and `focused` are the page's
  `visibilityState` and `hasFocus`, handed to the reporter through the module's `activityFacts` op when they change
  (`reportWindowFacts`). The reporter holds its first report until they arrive and reports again on each change, as
  the reference's visibilitychange, focus and blur listeners do. `T3ActivityReporter.swift` keeps only the pointer,
  key and wheel interactions (the reference's window listeners).
- **Other `window` focus listeners of the reference** that the clone had left out for lack of the fact: the workspace
  card asks `vcs.refreshStatus` when the window regains the focus (GitActionsControl, `refreshVcsOnFocus`), and
  SnapShot settings read the permissions again on focus (SnapShotSettings `refreshState`).

Live (agent, lane server through a trace proxy): Before, under the agent every report said `focused:false` (the
AppKit reading of an agent window). After, `focused:true`; `prefer has-focus false` gives a report with
`focused:false`, `shell.notifications` "… (window inactive)" and `page.hasFocus` false; focus back gives
`vcs.refreshStatus` and a report with `focused:true`. Kept: the Dock badge, its clearing on focus, the click that opens
the thread and the sounds (`T3Notifications.swift`), for #224. Not built here (the clone had no counterpart): the
reference's toast timers that run only while visible and focused (git and provider success toasts), DiffPanel's
refresh on focus, the composer's refocus on window focus, SnapShotCoordinator's drain on focus. They can be built on
`page.hasFocus` now. The issue stays open for #224.
