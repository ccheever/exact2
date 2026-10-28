surfaces: android
agent: Claude Code (Claude Opus 5.5)
task: receive text and files from other apps' share sheets on Android (LLP 1133.024 stage 3)

### Rough

- Fieldnotes had no Android crate; one copied from Excord's and the Linux crate's
  build took about ten minutes to write and eight to build under the Mac's load. Its
  first APK build stopped until the Gradle lock and checksums were written
  (`--write-locks`), since the pedometer package names a Maven library.
- Every shared file was dropped on the first run. The reader handed exact2 each stream
  as `/proc/self/fd/<n>`, and exact2 opened that path again; for a MediaStore file the
  path leads through the sender's storage, which the intent's grant does not open. The
  copy now reads from the descriptor itself, with the size the reader read from it.
  About 30 minutes, most of it rebuilding.
- `adb shell am start` cannot grant a stream extra on its own: the drive passes the
  same MediaStore URI as the intent's data so `--grant-read-uri-permission` covers it.

### Lean in

- The claim path built for the other hosts took Android's share unchanged once the
  reader handed it over in the driver's `{"target", "share"}` shape.
- The agent's reader relay drove the app between intents, so the drive could send a
  share from the shell and read the tree back.

### Needed

- share target (receiving): provided, new on Android (LLP 1133.024 D5)

### Checkpoints

- Android build: rough (the lock files, then the build's time under load)
- driving the app (`share-android.drives.mjs`): smooth once the copy read the descriptor
