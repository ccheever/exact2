# @exact/gamepad: the device checklist

What a person checks with real pads, which no drive here can: an agent
never touches a controller attached to the machine it runs on (LLP
1133.022 §21). The drives under the agent use the declared substitute and,
on Linux, Windows, Apple and Android, a drive's injected pad
(`EXACT_AGENT_PADS`); these steps are the hardware half.

You need a DualSense (PS5) and an Xbox Wireless Controller, each paired
over Bluetooth or plugged in by USB, and the Beacons game
(`game/games/beacons`) built for the platform under test. Record each
step's result and the OS version.

## Every platform with an implementation

1. Start Beacons with no pad connected and tap Play. The touch controls
   (Move, Jump, Light) show. Walk to a beacon: the prompt reads "Press E
   or Light to light this beacon".
2. Connect the DualSense. Press any button. The touch controls go.
   Near a beacon the prompt reads "Press Square to light this beacon".
3. Press Square near the beacon. It lights, and the pad rumbles once,
   briefly (about a fifth of a second), as it lights: not before, and not
   noticeably after.
4. Light the other two beacons. Each rumbles; the third also turns the
   DualSense's light bar amber where the platform lets an app set it (see
   the table below). Tap Play again: the light bar returns to the
   system's colour.
5. Disconnect the DualSense (switch it off, or unplug it). The touch
   controls come back and the prompt drops the pad's word.
6. Repeat 2 to 5 with the Xbox pad: the prompt reads "Press X to light
   this beacon"; it rumbles; its light does not change (it has none an
   app can set).
7. Connect both. Press a button on the Xbox pad, then light a beacon with
   it: only the Xbox pad rumbles. Press a button on the DualSense, light
   one with it: only the DualSense rumbles.
8. While a pad rumbles, light another beacon at once (two within 180 ms):
   the rumble restarts rather than doubling; nothing keeps running after.
9. Put the app in the background and light nothing: no pad rumbles.
   Return: each pad keeps its index, and the prompt still names the pad
   in use.
10. The pad still moves the player and jumps exactly as before this
    change (QUEUE's existing pad checks).

## Per platform

| Platform | Built here | Rumble | Light bar | Notes |
| --- | --- | --- | --- | --- |
| Linux (desktop or the NucBox) | yes: evdev `FF_RUMBLE`, the registry in `host/linux/src/pads.rs` | yes, where `/dev/input/event*` opens read-write (the `input` group) | no: `setLight` answers `NotSupportedError` | A pad opened read-only lists no effects; check that `effects` is empty then. |
| Windows (x64, ARM64) | yes: XInput, `host/windows/src/pads.rs` | yes, on an XInput pad (the Xbox pad) | no | The DualSense is not an XInput pad: it lists no pad on Windows. |
| Web (Chrome, Edge) | yes: `web/index.js` | yes, through `vibrationActuator` | no | Safari and Firefox list no effects. |
| macOS, iOS, iPadOS, tvOS | yes: `apple/Gamepad.swift` over `GCController`, the slot table shared with the canvas (`ExactPads`) | yes, through each handle's Core Haptics engine (`GCDeviceHaptics`); the DualSense's triggers take `trigger-rumble` | yes on the DualSense and DualShock 4 (`GCDeviceLight`); the colour before the app's first write comes back on `setLight()`, at the background and when the session ends | The prompt shows the button's SF Symbol beside its name (step 2). An Xbox pad lists `dual-rumble` only. |
| Android, Android TV | yes: `android/Gamepad.kt` over the process's `Pads` and `InputDevice` | yes, through the device's `VibratorManager` (Android 12 and later; one vibrator below it); the app's manifest carries `VIBRATE` | where the device has an RGB light (`LightsManager`, Android 12 and later); closing the session gives it back | Also check step 9 with Developer options' "Don't keep activities" on: the pad keeps its index and the prompt its word when the activity is made again. |

On Linux and macOS, also check `bun scripts/agent.mjs <linux|macos> state`'s
`extensions.gamepad` while playing: the pads listed have the right
`family`, and each effect is listed with `origin: app` and `result`
`complete`.
