# @exact/geofencing: drives by hand

What the agent cannot drive: a fence crossed with the app terminated, the
second ("Change to Always Allow") prompt, fences after a reboot, and
anything on an Android phone (LLP 1133.021 §14 stages 2 and 3). The
geofencing fixture (`apps/geofence-fixture`) watches Palo Alto station
(37.4434, −122.1649, 200 m) and posts a notification on arrival; Caltrain's
own switch waits for `@exact/notifications`. Use a test phone (LLP
1133.020), never a personal one, and move it by simulated location where
the steps say so: Xcode's Debug ▸ Simulate Location with a GPX file, or
walking to the station. Record the date, the OS version, the device and
what happened beside each step.

A GPX file for Xcode, `paloalto.gpx`, with a point 2 km away then the
station:

```xml
<gpx version="1.1"><wpt lat="37.4600" lon="-122.1800"/><wpt lat="37.4434" lon="-122.1649"/></gpx>
```

## iPhone (iOS 17 or later)

`bun host/apple/build.mjs --ios --device --run geofence-fixture-apple`.

1. **Turn it on, fresh install.** Tap Off beside "Next trains when I
   arrive". See iOS's location prompt with the fixture's purpose ("…even
   while it is closed"). Choose "Allow While Using App". The switch reads
   On; "fences: 1". The status reads `true granted` (provisional Always).
2. **Arrive, running.** Simulate a location 2 km away, then the station.
   Within a few minutes see "entered station:pa at …" and a notification
   "Arrived".
3. **Arrive, terminated.** End the app from Xcode (Stop), so it is
   terminated but not force-quit (step 5 is the force-quit). Move away,
   then to the station. See the notification without opening the app.
   This is the first event the app can hear: iOS shows its second prompt,
   "Change to Always Allow", around then. Choose Always. Record whether the
   notification came before or after the prompt was answered.
4. **Keep When In Use.** Repeat 1 to 3 on a fresh install but choose "Keep
   Only While Using" at the second prompt. Open the app: the status reads
   `true denied` and the explanation shows.
5. **After a force-quit.** With Always, force-quit the app from the app
   switcher, move away and back. Record whether a notification came.
6. **After a reboot.** With Always and the fence on, restart the phone and
   unlock it. Move away and back. Record whether a notification came, and
   open the app: "fences: 1".
7. **Off.** Tap On: "fences: 0". Moving to the station posts nothing.

## Mac (macOS 14 or later)

`bun host/apple/build.mjs --run geofence-fixture-apple`. Turn it on; macOS
asks for location once. A crossing is heard only while the app runs and
the Mac is awake (D14); a Mac rarely moves 200 m, so this step is the
prompt and the status only.

## Android phone with Google Play services (Android 11 or later)

See `packages/geofencing/test/android.mjs` for what the build checks. The
fixture's Android build is that script's.

1. **Two decisions.** Turn it on. See the location dialog ("While using the
   app"); allow. Then the app's location settings page opens: choose
   "Allow all the time" and go back. The switch reads On.
2. **Arrive, running.** With a mock location app (Developer options ▸
   Select mock location app), move away then to the station. Within a few
   minutes, "entered station:pa".
3. **Arrive, closed.** Close the app (swipe it away), move away and back.
   Open the app: the arrival is delivered then, with its original time
   (Android's headless session is not built, D14 step 3).
4. **Location off and on.** Turn location off, then on, with the app
   closed; move away and back: nothing (Play services dropped the fence).
   Open the app (it re-arms), move away and back: heard.
5. **Reboot.** Restart the phone; without opening the app move away and
   back; open the app: the arrival is delivered (the boot receiver
   re-armed the fence).
