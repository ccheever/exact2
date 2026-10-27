# @exact/calls: drives by hand

What the agent cannot drive (LLP 1133.033 §9.3, drives 11 to 13): a phone
locked with a take running, the system's call screen, a cellular call put
through during a test call, and Android's notifications. Record the date,
the device, the OS version and what happened beside each step when you
run it. Use a test phone, never a personal one.

**What is built.** The foreground service on Android and the background
line on iOS (§12 stages 0 and 3), so the recorder's steps run now. Calls
are the declared substitute on every target: the call is the module's own
state, with no system screen (`callsStatus().systemUI` is false). CallKit
(stage 2), Core-Telecom (stage 4) and Media Session's call actions (stage
1) are not built, so the steps below wait for them; they are written here so
the person running them knows what to look for when they land.

## The recorder

The foreground service's steps are the recorder's:
`packages/recording/test/device.md` items 4 (a take locked for five
minutes, the Android notification, `dumpsys`, and the same without the
line) and 10 (a take the Task Manager stopped, recovered at the next
launch).

## Calls (wait for stages 1, 2 and 4)

Harmony's **Make a Test Call**, under You, on an iPhone and a Mac; the
package's fixture on an Android phone (`packages/calls/test/fixture`,
when it lands).

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
