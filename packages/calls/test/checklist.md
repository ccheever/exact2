# @exact/calls: drives by hand

What the agent cannot drive (LLP 1133.033 §9.3, drives 11 to 13): a phone
locked with a take running, the system's call screen, a cellular call put
through during a test call, and Android's notifications. Record the date,
the device, the OS version and what happened beside each step when you
run it. Use a test phone, never a personal one.

**What is built.** The foreground service on Android and the background
line on iOS (§12 stages 0 and 3), so the recorder's steps run now. The
runner keeps the call list on every target. On iOS CallKit shows it
(stage 2, steps 1 to 3); on the web Media Session's call actions are its
controls (stage 1, step 5); on Android Core-Telecom and a `CallStyle`
notification (stage 4, step 4); a Mac has no system call screen.

## The recorder

The foreground service's steps are the recorder's:
`packages/recording/test/device.md` items 4 (a take locked for five
minutes, the Android notification, `dumpsys`, and the same without the
line) and 10 (a take the Task Manager stopped, recovered at the next
launch).

## Calls

Harmony's **Make a Test Call**, under You, on an iPhone and a Mac; the
package's fixture (`packages/calls/test/fixture`) on an Android phone.

1. **The call screen (iPhone).** Make a test call: the green call
   indicator shows, "Say something" for five seconds, then the take plays
   back and the call ends. The call is not in the Phone app's Recents.
2. **Ended from the system.** During the take, end the call from the
   Dynamic Island or the lock screen: the take stops and nothing plays.
3. **A cellular call through it.** During the take, call the phone from
   another phone and accept: the test call holds and the take pauses; hang
   up the cellular call and resume the test call: the take records again.
4. **Android's call.** In the fixture, Make a Test Call: an ongoing
   `CallStyle` notification shows. `adb shell dumpsys activity services
   <package>` lists `ForegroundService$Call` as `phoneCall|microphone`.
   Ring me in 10 s, then lock the phone: the call rings full screen over
   the lock screen; with full-screen intents turned off for the app in
   Settings, a heads-up notification instead. Answer from the
   notification: the take records with the app hidden.
5. **The browser's call controls (Chrome on a Mac).** Make a test call
   in Harmony's web build and, during the take, open Chrome's media
   controls (the toolbar's media button). Chrome shows the call actions in
   a Picture-in-Picture window, which Harmony does not open (D19), so
   record what the media controls show. Where a microphone or hang-up
   button shows, the microphone mutes the call ("Muted" shows) and hang-up
   stops the take with nothing played. `bun packages/calls/test/web.mjs
   chromium` drives the same handlers without the browser's buttons.
