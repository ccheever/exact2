# @exact/motion on a phone

What the agent cannot drive: real sensors. The iOS simulator reports no
motion hardware, the Android emulator's are virtual, and under the agent
the runner's substitute answers every name unless the drive binds the
implementation (`--bind motion`), so these checks need a person and a
device (LLP 1133.013 §10 drive 7, §13). Each item says what to see;
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
2. **Screen frame.** In landscape, tap **Screen frame**. With the screen
   facing up, lower its right edge and raise its left edge: x becomes
   negative, as in portrait. Gravity uses the upward acceleration and
   the screen's x-axis points right ([Accelerometer §6.3–6.4](https://www.w3.org/TR/accelerometer/#gravity-sensor)).
3. **Gyroscope.** Spin the phone flat, counter-clockwise seen from above:
   `spin`'s z is positive, in rad/s (about 3 for half a turn a second).
4. **Batch.** `batch` reads about six samples an answer and `dropped` 0;
   `counted` the same.
5. **Background gap.** Leave the app and return: the next `batch` ends in
   `true` (interrupted).
6. **Permission.** In Safari, `status` reads `prompt` until **Ask**,
   which shows one prompt; allow it and `asked` reads `granted`. In the
   apps and Chrome, `status` reads `granted` and **Ask** shows nothing.

## Beacons (iPhone, Android phone)

Build and install on an iPhone: `bun game/prove.mjs beacons --hosts ios
--device`, or `bun host/apple/build.mjs --ios beacons-apple --device --run`.
On an Android phone: `bun scripts/agent-android.mjs build beacons --apk`
(an NDK of clang 18 or newer in `ANDROID_NDK_HOME`, a JDK 17 or newer in
`JAVA_HOME`), then `adb install -r` the APK it names. Tap **Play**, then
**Pause**.

On Android, `adb logcat -s exact` shows what the module does: a canvas
input logs `motion: input gravity on world at 60.0 Hz` when it starts,
and the gravity sensor's rate follows it (`motion: sensor 9 at 60.0 Hz`
while the world reads, `30.0 Hz` when only the level does).

7. **The toggle.** Turn on **Tilt to move**: the app asks nothing.
8. **Level.** Hold the phone at a comfortable angle: the dot glides to the
   tilt and the readout gives the two angles. Tap **Set level**: it reads
   `0° · 0°` and the dot centres.

Items 9 to 11 hold the phone at the angle **Set level** made level, then
tilt from there. Within 2° of it the stick reads zero (D6's dead zone);
20° gives the full stick, which walks at 4 m/s.

9. **Walk.** Tap **Resume**, then tilt: tilting the right edge down walks
   the player right, tilting the top edge away walks it forward, and
   holding the level angle stands it still. If either direction is
   reversed, the sign of D6's map against the platform's gravity is wrong:
   say which and on which phone. Screen-frame gravity reads negative x
   when the right edge is down; D6 negates that roll to walk right.
   If the player does not move at all on
   Android, look for the `motion: input` line: without it the start was
   refused; with it, the samples did not reach the world.
10. **Pause and tilt off.** Tap **Pause**: the player stops where it is
    and tilting moves nothing. Turn tilt off and tap **Resume**: the
    player stays still however the phone is held, and the on-screen stick
    moves it.
11. **Background.** With tilt on and the game running, leave the app for a
    few seconds and come back: tilting walks again within a frame or two,
    and the player does not jump to where the time away would have taken
    it. Rotate the phone to landscape and back, where the app allows it:
    tilting the screen's right edge down still walks right.
12. **Smoothness.** The dot moves smoothly between readings, not in steps.
13. **The screen keeps its orientation while tilt is on** (LLP 1133.029
    §9.1 drive 9). With tilt on and the game running, steer hard to one
    side and lay the phone nearly flat: the screen does not turn, however
    far the phone is tilted. Turn tilt off: note whether the screen turns
    to the way the phone is held within a second or only when the phone is
    next turned (the iOS 27 simulator does the second). Turn tilt on while holding the phone
    in landscape, then stand it upright: it stays landscape. On an iPad on
    iPadOS 26 or later, the same in full screen; in Slide Over or a
    resized window the system lifts the lock and the screen turns, which
    is iPadOS's own behaviour. On an Android tablet (600 dp or wider) the
    lock holds as on the phone.

Beacons on the web has no tilt yet: the web's wasm host, which games run
on, has no extension modules (QUEUE.md).
