# @exact/gamepad: the device checklist

What a person checks with real pads, which no drive here can: an agent
never touches a controller attached to the machine it runs on (LLP
1133.022 §21). The drives under the agent use the declared substitute and,
on Linux and Windows, a test build's injected pad (`EXACT_AGENT_PADS`);
these steps are the hardware half.

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
| macOS, iOS, iPadOS, tvOS | not yet (`GCController`, Core Haptics) | — | — | Runs under the substitute until the Swift implementation lands. |
| Android | not yet (`InputDevice`, `VibratorManager`) | — | — | Runs under the substitute until the Kotlin implementation lands. |

On Linux, also check `bun scripts/agent.mjs linux state`'s
`extensions.gamepad` while playing: the pads listed have the right
`family`, and each effect is listed with `origin: app` and `result`
`complete`.
