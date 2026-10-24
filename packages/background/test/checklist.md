# @exact/background: drives by hand

What the agent cannot drive: the system's own network changes, a task
that iOS launches, and a phone (LLP 1133.017 §11, drive 9). Each step says
what to see. Record the date, the OS version and what happened beside each
step when you run it.

The host's lines are in the system's log, under subsystem `dev.exact`,
category `wakes` (Console.app, or `log stream --predicate 'subsystem ==
"dev.exact"'` on the Mac): which task arrived, how many requests were due,
whether a session was booted for it, and each transfer's ending.

## Harmony on an iPhone

Build and install a development build on the phone:
`bun host/apple/build.mjs --ios --device --run app-apple` with
`EXACT_APP_DIR=apps/harmony/app`, against a Snapback server the phone can
reach (`EXACT_BACKEND_ORIGIN`). `<bundle id>` below is the build's
(`CFBundleIdentifier` in its Info.plist).

1. **A message written offline goes out later.** Open #general and turn
   on Airplane Mode. Write a message: it shows pending. Leave the app
   (Home gesture) and turn Airplane Mode off. See the message arrive on
   another device at a later refresh or processing wake, or at once when
   you open Harmony again. Record how long it took, and which task the log
   says ran it.
2. **A refresh wake from the debugger.** Turn on Notify Me of Mentions
   under You. Leave the app. From another persona on another device, post
   a message that mentions this persona. In Xcode, attach to the app,
   pause, and run
   `e -l objc -- (void)[[BGTaskScheduler sharedScheduler] _simulateLaunchForTaskWithIdentifier:@"<bundle id>.refresh"]`,
   then continue. Apple says this works only on devices
   ([Starting and terminating tasks during development](https://developer.apple.com/documentation/backgroundtasks/starting-and-terminating-tasks-during-development)).
   See one notification for the room. Tap it: Harmony opens that room.
3. **A processing wake.** As 1, but leave the outbox's message pending
   (Airplane Mode on, write, leave, Airplane Mode off), then in the
   debugger run the same command with
   `@"<bundle id>.processing.network"`. See the message delivered, and the
   log's line `<bundle id>.processing.network: 1 due` (the processing twin
   of the `sync` request).
4. **The headless boot.** Turn on Notify Me of Mentions, leave Harmony,
   and stop it with Xcode's Stop button (the system's termination; a
   swipe from the app switcher is a force-quit, after which iOS launches
   no background task). Leave the phone on power and Wi-Fi, locked, for a
   few hours, posting a mention from another persona now and then. When a
   notification comes, read the log: `<bundle id>.refresh: 1 due`, `a
   refresh wake found no module ready; booting a session for it`, and
   `the module and the data open for the wake`, in a process the system
   launched (the debugger's `_simulateLaunchForTaskWithIdentifier:` runs
   in a live process, so it cannot drive this). Then open Harmony from the
   Home Screen: it shows its rooms with no wait on the launch screen, the
   session already up.
5. **Expiry.** As 2, with the server stopped so the round hangs, then run
   `e -l objc -- (void)[[BGTaskScheduler sharedScheduler] _simulateExpirationForTaskWithIdentifier:@"<bundle id>.refresh"]`.
   See no notification and no crash; the debugger console shows the task
   ended unsuccessful.
6. **Background App Refresh off.** In Settings > General > Background App
   Refresh, turn it off for Harmony. Open Harmony: Notify Me of Mentions
   is hidden, since the status reads `denied`, and a message written
   offline is still delivered at the next open.

## The transfer carrier on an iPhone

`packages/background/test/transfer.mjs ios --sim <udid>` drives this on a
simulator. On a phone it is measured by hand, since a phone's
`nsurlsessiond` and its relaunch policy are the device's (D10):

1. Build the probe fixture the suite makes (run the suite once with
   `--sim`, then build the same app directory for the phone with
   `--device`), and serve the suite's `/upload` from a Mac the phone
   reaches, holding its response 60 s.
2. Tap Upload u2, leave the app, and stop it with Xcode's Stop button
   while the response is held. When the server answers, see the log's
   `relaunched for <bundle id>.transfers`, `u2 0 ended 201`, and the
   wake's boot. Record whether the server's `/heard` saw `body "kept u2"`
   (the response body survived) or `bodyLost true`.
3. As 2, but restart the phone while the response is held, and do not
   unlock it until the server has answered. Record what the log says at
   the relaunch, and what `/heard` saw after the first unlock (D10's
   staging before the first unlock is not built: record whether the
   record could be written).
4. As 2, but swipe the app away in the app switcher. Record that the
   upload is cancelled (`aborted`) at the next launch.

## Harmony on a Mac

1. **The outbox while offline.** Turn Wi-Fi off. Write a message: it shows
   pending. Turn Wi-Fi on: the message is delivered within seconds
   (`NWPathMonitor`), with Harmony in front or behind other windows.
2. **A catch-up with the window closed.** Turn on Notify Me of Mentions.
   Close the window (Harmony keeps running). Post a mention from another
   persona. Within the scheduler's window (the tag's 30 minutes to half as
   long again, at the system's choice) see the notification; click it to
   open the room.

## Android

No Android implementation is built (§14, stage 3). An Android build of an
app that uses `@exact/background` gets the declared substitute: no wake
fires on a phone.
