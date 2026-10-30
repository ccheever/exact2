surfaces: android, web, ios
agent: Claude Code (Claude Opus 5.5)
task: drive the app's quick actions and URL arrival on an Android emulator

### Rough

- A URL sent to the running app never reached `onNewIntent`. The reader's
  activity was `standard`, so `am start -d /nowhere` stacked a second
  MainActivity over the first (two `Hist` entries in `dumpsys activity`), and
  an intent equal to the task's root only brought the task forward. Fixed in
  exact2: the activity is `singleTop` (`host/android/app/src/main/AndroidManifest.xml`).
  About 20 minutes.
- Opening the add sheet from that URL threw a `NullPointerException` in
  `TextView.checkForResize`, printed to `System.err` from `Fields.measure`:
  the field measurer, an `EditText` never attached, had no layout params,
  and a second `setText` after a measure reads them. Fixed in exact2 by
  giving it `WRAP_CONTENT` params. About 15 minutes.
- `agent-android.mjs build --apk` fails with "Gradle requires JVM 17 or
  later" when `JAVA_HOME` is a JDK 11; set it to a 17 by hand. The message
  is Gradle's own and says what to do.
- iOS: CoreSimulator stopped answering on the build Mac (`simctl list` hung
  for minutes, load average near 600), so the iOS drives were not run.

### Lean in

- `dumpsys shortcut` shows both the manifest's Add Book and the app's
  Continue *Piranesi* with their intents, so the shortcut list is checkable
  without the launcher.

### Needed

- quick actions: Exact (`@exact/quick-actions`, the manifest's `shortcuts`)
- a URL from outside the app: Exact (the navigation root)

### Checkpoints

- Launcher long press: Add Book and Continue *Piranesi*, both with a blank
  icon (no icon is given on Android yet).
- Add Book cold and the launcher's Continue *Piranesi* warm: the add sheet,
  then the book, with one activity and the same process.
