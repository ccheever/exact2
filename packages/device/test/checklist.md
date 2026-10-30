# The device module on a device (LLP 1133.031 §12 stages 2 to 4)

What the agent and the simulators cannot make the device do: run on a
battery, plug in and unplug, turn on Low Power Mode, Low Data Mode, Battery
Saver or Data Saver, and grow hot. Each step names what to see; a step that
shows something else is a finding. A person runs them, never an agent on a
phone (LLP 1133.020).

The apps are Weatherlight (`apps/weatherlight`: the sky pauses in Low Power
Mode and when the device is seriously hot, and refreshes every thirty
minutes instead of ten in Low Power Mode or Low Data Mode) and Expose
(`apps/expose`: the status bar's battery and Settings' Battery row). The
conformance fixture shows each fact as text:
`bun packages/device/test/fixture.mjs <dir>` makes it, and `bun exact.mjs`
in that directory builds it like any app.

## iPhone (iOS 17 or newer)

1. `bun host/apple/build.mjs --ios --device expose-apple --run`, on battery
   power. The status bar's battery fill matches the phone's own level, no
   bolt. Settings › Battery shows the same percentage as a whole number.
2. Plug in a cable: a bolt shows inside the mark within a few seconds.
   Unplug it: the bolt goes.
3. Turn on Low Power Mode (Control Center or Settings › Battery): the fill
   turns yellow, as the phone's own battery does. Turn it off: the fill
   returns to the ink colour.
4. `bun host/apple/build.mjs --ios --device weatherlight-apple --run`. The
   sky moves. Turn on Low Power Mode: the sky stops, and its button reads
   "Play sky animation". Tap it: the sky moves again although Low Power Mode
   is on. Turn Low Power Mode off and tap the button: the sky stops and
   stays stopped; the person's choice holds.
5. With Weatherlight in front, open Xcode › Window › Devices and Simulators,
   select the phone, and set Device Conditions › Thermal State to Serious,
   then Start ([Test under adverse device conditions](https://help.apple.com/xcode/mac/current/#/dev308429d42)):
   the sky stops. Set it to Fair: it moves. Stop the condition.
6. Settings › Wi-Fi › (the network) › Low Data Mode on. In the fixture
   (`--bind` is not needed outside the agent), `network` reads
   `wifi true`. Turn it off: `wifi false`. On cellular with Low Data Mode
   on for cellular data: `cellular true`.
7. Leave the app, change something (plug in, or turn Low Power Mode on),
   and return: the change shows on return (the foreground read, D5 item 4).

## Mac laptop (macOS 14 or newer)

8. `bun host/apple/build.mjs expose-apple --run` on battery: the fill
   matches the menu bar's battery. Plug in the charger: a bolt. On macOS 12
   or newer, System Settings › Battery › Low Power Mode "Always": the fill
   turns yellow, and Weatherlight's sky stops.
9. A Mac with no battery (a Mac mini or a Studio): the fixture reads
   `1 true` and Expose shows a full mark with a bolt, as Chrome does on a
   desktop.

## Android phone (Android 10 or newer)

10. `bun scripts/agent-android.mjs build weatherlight --apk`, install the
    APK, open it. Battery Saver on (quick settings): the sky stops. Off:
    it moves.
11. Settings › Network › Data Saver on, on a metered network (cellular, or
    a Wi-Fi network marked metered): the fixture reads `<type> true`. On an
    unmetered Wi-Fi network it reads `wifi false` with Data Saver on, since
    Data Saver restricts only metered networks.
12. Plug in a charger and unplug it: the fixture's `battery` shows `true`
    then `false`, the level in hundredths matching the status bar.
13. With adb, the platform's own commands set each fact; each changes the
    fixture's text without a relaunch:
    `adb shell dumpsys battery set level 42` (`0.42`),
    `adb shell dumpsys battery unplug` (`false`),
    `adb shell cmd power set-mode 1` (low power `true`),
    `adb shell cmd thermalservice override-status 3` (`serious`),
    then `adb shell dumpsys battery reset`, `cmd power set-mode 0` and
    `cmd thermalservice reset`.
14. `adb shell dumpsys package com.exact.weatherlight | grep ACCESS_NETWORK_STATE`
    shows the permission granted at install, with no prompt shown.

## Linux laptop

15. The fixture's Linux build (`bun exact.mjs linux` in its directory), run
    on a laptop on battery: `battery` matches the desktop's battery
    indicator; plug in: `true`. Turn on Power Saver (GNOME's quick
    settings): `power` reads `true none`.

## Windows (the NucBox, in its signed-in console session)

16. The fixture's Windows build: a desktop with no battery reads `1 true`,
    `ethernet false` on its wired network. Settings › System › Power ›
    Energy Saver on: `power` reads `true none`; off: `false none`.
