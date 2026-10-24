# @exact/motion on a phone

What the agent cannot drive: real sensors. The iOS simulator reports no
motion hardware, the Android emulator's are virtual, and under the agent
the runner's substitute answers every name, so these checks need a person
and a device (LLP 1133.013 §10 drive 7, §13). Each item says what to see;
note the device, the OS version and what you saw where it differs.

## The module (`test/fixture`)

Make the fixture app as the suites do (`fixture.mjs`: `exact new`, the
fixture's files, this package by `file:`), then build it for each target:

| Target | Build and run |
|---|---|
| iPhone, the app | `bun exact.mjs ios --device --run` in the fixture app |
| iPhone, Safari | `bun exact.mjs web-build`, then `target/web-dist` served over HTTPS |
| Android phone, Chrome | the same dist |
| Android phone, the app | the fixture given a Canvas library as `test/android.mjs` gives it, then `bun <exact2>/scripts/agent-android.mjs build <name> --apk` with `EXACT_APP_DIR` set to it, and `adb install` the APK it names |

1. **Units and sign.** Face up on a table: `gravity` reads about
   `0 0 9.8` (m/s²). Stood upright in portrait: about `0 9.8 0`. If Safari
   reads `0 0 -9.8`, the web implementation's WebKit negation is wrong for
   this version: say which.
2. **Screen frame.** In landscape, tap **Screen frame**: tilting the
   screen's right edge down makes x positive.
3. **Gyroscope.** Spin the phone flat, counter-clockwise seen from above:
   `spin`'s z is positive, in rad/s (about 3 for half a turn a second).
4. **Batch.** `batch` reads about six samples an answer and `dropped` 0;
   `counted` the same.
5. **Background gap.** Leave the app and return: the next `batch` ends in
   `true` (interrupted).
6. **Permission.** In Safari, `status` reads `prompt` until **Ask**,
   which shows one prompt; allow it and `asked` reads `granted`. In the
   apps and Chrome, `status` reads `granted` and **Ask** shows nothing.

## Beacons (iPhone)

Build and install: `bun game/prove.mjs beacons --hosts ios --device`, or
`bun host/apple/build.mjs --ios beacons-apple --device --run`. Tap
**Play**, then **Pause**.

7. **The toggle.** Turn on **Tilt to move**: the app asks nothing.
8. **Level.** Hold the phone at a comfortable angle: the dot glides to the
   tilt and the readout gives the two angles. Tap **Set level**: it reads
   `0° · 0°` and the dot centres.

Items 9 to 11 wait on the context's `surfaces.input`, which no host gives
an implementation yet: on a phone `startSensorInput` answers
`NotSupportedError`, so the player does not walk by tilt (LLP 1133.013
§13, stage 2). Run them once it lands.

9. **Walk.** Tap **Resume** and tilt: tilting the right edge down walks the
   player right, tilting the top edge away walks it forward. If either is
   reversed, the sign of D6's map against the platform's gravity is wrong:
   say which.
10. **Pause and tilt off.** Pause, turn tilt off, resume: the player stops,
    and the on-screen stick moves it.
11. **Background.** With tilt on, leave the app and come back: tilting
    still walks, and the player does not jump.
12. **Smoothness.** The dot moves smoothly between readings, not in steps.

Beacons on the web and on Android has no tilt yet: the web's wasm host,
which games run on, has no extension modules, and an Android game binds
none (QUEUE.md).
