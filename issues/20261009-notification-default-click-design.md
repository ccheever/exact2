# Notifications: a click runs an app action, and an app badge (rest of #114)

**Status:** Open
**Systems:** notifications, Contract, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/224

## Current scope

Obtain click-only scoped DEFERRED ruling before code, then specify tag identity/cold-warm delivery/focus through existing agent operations. Badges, push, richer actions/replies, repeats and permission facts remain deferred.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
The rest of #114, split out because #114 closed when its focus fact landed (#219, `exactPage().hasFocus`). Two things remain, and `rules/DEFERRED.md` refuses both by name, so they need a waiver before any code:

- **A click on a notification runs an app action.** Today `showNotification(title=, body=, tag=)` posts a notification, but a click on it reaches nothing in the app. `host/apple/Sources/ExactKit/Notify.swift` implements only `willPresent`, with no `didReceive`. The web glue sets no `onclick`.
- **An app badge.** There is no `setAppBadge`/`clearAppBadge` host command (`type-unknown-command`).

## Blocked by
`rules/DEFERRED.md:407-408`: "Still refused: push delivery from a server, actions and replies on a notification, badges, repeating schedules, and a readable permission fact."

One point to settle in the ruling: "actions" may mean a notification's action buttons and inline replies (`UNNotificationAction`, the web's `actions` option), not its default click. If a plain click is not covered by the refusal, only the badge needs a waiver.

## Options
- **A.** Add a root event `notificationclick=act(tag)`. The host brings the window forward, then runs the action with the notification's `tag`. Add host commands `setAppBadge(n)` and `clearAppBadge()`: the Dock tile on macOS, the icon badge on iOS, `navigator.setAppBadge` on the web where supported; refused on Linux. Agent: `tap @notification <tag>` delivers a click, and `state` reports the badge.
- **B.** Only the click (if it is ruled outside "actions"); badges stay refused.
- **C.** No change. Document the native-module pattern (a `UNUserNotificationCenter` delegate plus `NSApp.dockTile`, about 185 lines of Swift per app).

## Recommendation
A. Both follow web APIs (`notificationclick`, the Badging API), cost about a day per host, and replace per-app native code that the agent cannot drive. Consumer: the T3 Code clone (it opens the finished thread from a click and counts unread notifications).

## Acceptance (if admitted)
As in #114's option A: an agent-driven click delivers the tag to the action on macOS and the web, an attended macOS click brings the window forward, and `setAppBadge(3)`/`clearAppBadge()` show and clear "3" on the Dock tile.

Related: #114 (closed; its focus fact landed in #219).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:51Z

**Decision: Propose default notification clicks first; keep badges and richer actions deferred.**

Keep open with the bounded scope below.

A click that opens the finished thread completes the reminder consumer better than adding badges. Current DEFERRED text must be clarified before treating a click as outside its actions refusal.

Ask for a click-only scoped ruling, then specify cold/warm delivery and tag identity. Retain the refusals on push, action buttons/replies, repeating schedules and permission facts.
