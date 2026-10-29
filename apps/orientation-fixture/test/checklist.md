# `orientationlock` on a device

What the agent cannot judge: whether the platform holds a lock when a
person turns the device. Under the agent the lock is applied for real, so
the simulator's turns are driven (`drive.mjs`), but a physical phone's
sensor, a person's rotation lock, an iPad's multitasking and Android 16's
large-screen rule need a person and a device (LLP 1133.029 §9.1 drive 9).
Each item says what to see; note the device, the OS version and what you
saw where it differs.

The fixture app is `apps/orientation-fixture`, whose manifest says no
`orientation`, so a phone allows portrait and both landscapes and a
tablet all four.

| Target | Build and run |
|---|---|
| iPhone, iPad | `bun host/apple/build.mjs --ios orientation-fixture-apple --device --run` |
| Android phone or tablet | `bun scripts/agent-android.mjs build orientation-fixture --apk`, then `adb install -r` the APK it names (`ANDROID_NDK_HOME` an NDK of clang 18 or newer, `JAVA_HOME` a JDK 17 or newer) |
| Chrome on Android | `bun host/web/build.mjs orientation-fixture`, its `target/web-dist` served over HTTPS; add it to the home screen or open it, then enter fullscreen from the browser's menu |
| Windows convertible or tablet | the Windows host's build of the fixture, as LLP 1115.004 builds an app for the NucBox |

Turn the system's own rotation lock off before starting.

## Phones and tablets

1. **A type holds.** Tap **Portrait** (the page): turn the device to
   landscape; the screen stays portrait. Tap **None**: it turns within a
   second.
2. **A type turns the screen.** Hold the device in portrait and tap
   **Landscape**: the screen turns to landscape at once, and stays
   landscape when the device is stood upright. Tap **None**.
3. **Current.** Hold the device in landscape and tap **Current**: stand it
   upright; the screen stays landscape. Tap **None**, still upright, and
   note whether the screen turns to portrait within a second or only when
   the device is next turned. The iOS 27 simulator does the second.
4. **A sheet over a locked page.** Tap **Portrait**, then **Sheet**: the
   sheet locks landscape and the screen turns. Tap the sheet's **None**:
   the page's portrait holds again under it. Tap the sheet's
   **Landscape**, then **Sheet over it** (a sheet with no lock): the
   screen stays landscape. Close both sheets: portrait.
5. **A dialog over a locked screen.** Tap **Landscape**, then **Dialog**:
   the screen stays landscape while the dialog is up, and after **Close**.
6. **Nesting.** Tap **None** (the page), **Root portrait**, then
   **Landscape screen**: the pushed screen locks landscape over the root's
   portrait. **Back**: portrait. Tap **Any** (the page): the device turns
   freely though the root says portrait. Tap **Root none**.
7. **A conflict.** Tap **Portrait** (the page) and **Side landscape**:
   neither holds and the device turns freely. Tap **Side none**.

On an **iPad** on iPadOS 26 or later, in full screen, only item 3 holds
(`current`): items 1, 2, 4 and 6 turn freely, which is iPadOS's answer to
an app that supports multitasking. In Slide Over or a resized window,
item 3 turns too. On an **Android tablet** (600 dp or wider) on Android 16
or later, likewise only item 3 holds.

On **Chrome on Android**, in fullscreen, items 1 to 3 hold; leaving
fullscreen lets the screen turn, and entering it again holds the lock
again.

## Windows

On a convertible or a tablet, items 1 and 2 hold when the screen is
turned. A desktop monitor does not turn by itself, so there `state
.orientationlock.applied.preference` is what to read.
