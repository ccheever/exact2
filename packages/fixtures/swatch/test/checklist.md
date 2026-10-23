# @fixture/swatch on a phone

What the suites cannot do: a person's touch on a real device. Build the
fixture as the suites make it (`fixture.mjs`), then for each phone:

- **iPhone**: `bun host/apple/build.mjs --device --ios <name>-apple --run` with
  `EXACT_APP_DIR` set to the fixture.
- **Android**: `bun packages/fixtures/swatch/test/android.mjs` builds the APK;
  install it with `adb install -r` from `host/android/app/build`.

On each:

1. The last line under the buttons is empty.
2. Tap **Show**. A red rounded box appears; the line reads `made N`,
   `displayed N true app` and `bound N tint app` (any order), the same `N`.
3. Tap **Recolor**. The box turns blue; a second `bound N tint app` is added.
4. Tap **Hide**. The box goes; `displayed N false app` is added. Tap
   **Unhide**: the box returns and a second `displayed N true app` is added.
5. Tap the box. The line below reads `tap`.
6. Tap **Remove**. The box goes; a second `displayed N false app` is added.

On Android a tap reaches a hosted view only as a click (`Natives.click`); a
gesture does not until LLP 1133.008 D8 item 6.
