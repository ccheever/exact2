# @exact/background: drives by hand

What the agent cannot drive: the system's own network changes, a refresh
task that iOS launches, and a phone (LLP 1133.017 §11, drive 9). Each step
says what to see. Record the date, the OS version and what happened beside
each step when you run it.

The headless boot is not built (§14, stage 2), so on iOS a wake reaches
Harmony only while its process is alive in the background: leave it with
the Home gesture, never swipe it away from the app switcher.

## Harmony on an iPhone

Build and install a development build on the phone:
`bun host/apple/build.mjs --ios --device --run app-apple` with
`EXACT_APP_DIR=apps/harmony/app`, against a Snapback server the phone can
reach (`EXACT_BACKEND_ORIGIN`).

1. **A message written offline goes out later.** Open #general and turn
   on Airplane Mode. Write a message: it shows pending. Leave the app
   (Home gesture) and turn Airplane Mode off. See the message arrive on
   another device at a later refresh wake, or at once when you open
   Harmony again. Record how long it took.
2. **A refresh wake from the debugger.** Turn on Notify Me of Mentions
   under You. Leave the app. From another persona on another device, post
   a message that mentions this persona. In Xcode, attach to the app,
   pause, and run
   `e -l objc -- (void)[[BGTaskScheduler sharedScheduler] _simulateLaunchForTaskWithIdentifier:@"<bundle id>.refresh"]`,
   then continue. Apple says this works only on devices
   ([Starting and terminating tasks during development](https://developer.apple.com/documentation/backgroundtasks/starting-and-terminating-tasks-during-development)).
   See one notification for the room. Tap it: Harmony opens that room.
3. **Expiry.** As 2, with the server stopped so the round hangs, then run
   `e -l objc -- (void)[[BGTaskScheduler sharedScheduler] _simulateExpirationForTaskWithIdentifier:@"<bundle id>.refresh"]`.
   See no notification and no crash; the debugger console shows the task
   ended unsuccessful.
4. **Background App Refresh off.** In Settings > General > Background App
   Refresh, turn it off for Harmony. Open Harmony: Notify Me of Mentions
   is hidden, since the status reads `denied`, and a message written
   offline is still delivered at the next open.

## Harmony on a Mac

1. **The outbox while offline.** Turn Wi-Fi off. Write a message: it shows
   pending. Turn Wi-Fi on: the message is delivered within seconds
   (`NWPathMonitor`), with Harmony in front or behind other windows.
2. **A catch-up with the window closed.** Turn on Notify Me of Mentions.
   Close the window (Harmony keeps running). Post a mention from another
   persona. Within the scheduler's interval (30 minutes, at the system's
   choice) see the notification; click it to open the room.

## Android

No Android implementation is built (§14, stage 3). An Android build of an
app that uses `@exact/background` gets the declared substitute: no wake
fires on a phone.
