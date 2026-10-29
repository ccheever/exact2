# @exact/alarms: drives by hand

What the agent cannot drive: a real ring through silence and Focus, the
Lock Screen, a restart before first unlock, and a phone (LLP 1133.030
§14). Use LLP 1133.020's test phones, never a personal one. Record the
date, the OS version, the device and what happened beside each step when
you run it.

## iPhone (iOS 26 or later): Metronome's practice timer

Build and install a development build on the phone:
`bun host/apple/build.mjs --ios --device --run metronome-apple`. The
widget extension that draws the countdown is signed with its own profile,
for the App ID `com.exact.metronome.widgets`; register it beside the
app's if the build asks.

1. **The permission.** On a fresh install, tap 1 of the lengths and Start.
   See AlarmKit's alert asking to allow alarms, with Metronome's text
   ("practice-timer-purpose"). Allow. See the row count down.
2. **Silent mode and a Focus.** Turn the ring/silent switch (or the Action
   button) to silent and turn on a Focus (Do Not Disturb). Start a 5-minute
   practice and lock the phone. See the Lock Screen's countdown with the
   title and the time left, counting down. At the end, hear the alarm
   sound at full alarm volume despite silent mode and the Focus, and see
   the full-screen alert with Stop and Repeat.
3. **Repeat.** Press Repeat. See a new 5-minute countdown on the Lock
   Screen. Unlock and open Metronome: the row shows the same time left as
   the Lock Screen, within a second.
4. **Stop.** At the second ring press Stop. Open Metronome: the row is the
   chooser again.
5. **The metronome playing.** Start the metronome, then a 5-minute
   practice, and lock the phone. Hear the clicks until the ring; at the ring
   hear only the alarm; after Stop hear nothing (the transport stopped and
   does not resume). Open the app: Start shows, the transport stopped.
6. **Pause from the app.** Start a practice, tap Pause after a minute.
   See the Lock Screen and the Dynamic Island show "Paused" with the time
   left. Tap Resume: the countdown continues from there.
7. **Cancel during a ring.** Start a 5-minute practice. When it rings,
   unlock and tap Cancel in the app (the row shows "Time is up"). The ring
   stops. On the iOS 26.5 simulator `cancel(id:)` ended the ring
   (the log line "alarms: cancel ended practice's ring"); record which line
   the phone logs (`log stream --predicate 'subsystem == "dev.exact"'`).
8. **A restart before first unlock.** Start a 10-minute practice, restart
   the phone and do not unlock it. See no countdown on the Lock Screen
   (the system shows none before first unlock), and at the end hear the
   alarm and see its alert anyway.
9. **Turned off in Settings.** In Settings, Metronome, turn Alarms off.
   Open Metronome: the row says alarms are turned off and shows Open
   Settings, which opens Metronome's settings page.
10. **A paired Apple Watch** (if the test phone has one). At a ring, see
    the Watch show the alert and stop it there; the phone stops too.

## iPhone on iOS 17 or 18

Install the same build. See no Practice row: `alarmStatus().available` is
false below iOS 26.

## Android 14 phone, fresh install: the fixture app

The fixture (`packages/alarms/test/fixture`) is the Android acceptance
drive (LLP 1133 D9 item 11 waived, LLP 1133.030 §15 question 6). Build it
into the reader APK with `bun packages/alarms/test/android.mjs` and install
the APK it names.

1. **The exact-alarm switch is off by default.** Tap Request. Allow
   notifications in the dialog. Android opens "Alarms & reminders" for the
   fixture with the switch off; turn it on and go back. The status reads
   `granted`.
2. **Do Not Disturb.** Turn on Do Not Disturb (with Alarms allowed, the
   default). Tap One-minute timer and turn the screen off. Hear the
   alarm sound at the end; wake the screen and see the notification with
   Stop and Repeat on the Lock Screen. `adb shell dumpsys alarm` lists the
   timer as an alarm clock; record whether the status bar's next-alarm
   icon for a timer reads as a tell beside the Clock app's own timer.
3. **Repeat, then Stop.** Press Repeat: the ring stops and a new
   one-minute countdown shows in the shade. At its end press Stop: the
   fixture's list is empty. Tap Alarm in a minute: at the ring the second
   button is Snooze.
4. **Swipe to stop.** Unlocked, during a ring, swipe the notification
   away: the ring stops and the alarm is gone.
5. **A reboot before unlock.** Set a PIN. Start a 3-minute countdown,
   restart the phone and do not unlock. Hear it ring at the end, and stop it
   from the Lock Screen notification.

## Android 12 or 13 phone

1. On Android 13, tap Request and deny notifications: the status reads
   `denied`, and Start is refused. Allow them in Settings: the status reads
   `granted`, since Android 13 gives exact alarms at install (Android 14
   does not).
2. On Android 12, the status is `granted` with notifications enabled;
   turn notifications off for the fixture and see `denied`.
3. With a countdown running, turn the fixture's Alarms channel off in its
   notification settings. At the end nothing rings, and the list shows the
   alarm gone.
4. A ring on Android 12 and 13 starts its service with no type (the
   manifest's `systemExempted` is Android 14's): hear the ring and see the
   notification, and record anything the log says of the service's type.
