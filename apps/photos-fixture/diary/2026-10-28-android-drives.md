surfaces: android
agent: Claude Code, Claude Opus 5.5
task: drive a photo library module and an image codec on Android emulators

### Rough

- The drive script called `agent.mjs android` without `--reader`, which drives the
  Linux binary and not the APK whose Kotlin carries the extension, and answered the
  read's ticket as `@photos.photoAssets`; on Android the ticket is held at the
  resource's node (`@recent`). The journal's `photos:` lines arrive in `logs`'
  `host` array (logcat), not `lines`. About 20 minutes.
- A Rust data source's storage request failed "storage requires an app storage
  adapter" until the source was wrapped in `exact_data_host::Storage`; under the agent
  it then failed "storage is unavailable in agent mode unless the drive names a
  scratch store" until the drive passed `--storage <name>`; and `transformImage` into
  `app:/tmp/edits/` failed "No such file or directory" until the source made the
  directory first. Each message named its fix except the last. About 40 minutes.
- With `--bind photos`, the launch's read was decided before the implementation was
  bound (after first pixel) and waited on the runner's agent ticket; the drive answers
  `tap @recent allow`. About 10 minutes.
- `clock settle` sometimes waits out its 20 s bound while a bound module's answer is
  in flight (QUEUE's "Android reader under the agent" line). It cost wall time, not
  results.
- The build script called `Fixture.grants()` on a unit struct; giving the struct a
  field broke the Android build only, after a seven-minute build, since the host's
  `cargo check` skips the bake. Check `--target aarch64-linux-android` first.

### Lean in

- Reading the edited files back with `adb root` on a `google_apis` image and checking
  pixels with `sips` and `scripts/png.mjs` made the codec's conformance a script.
- The reader's `layout` op gave the copy's drawn size, which caught a squashed photo
  without reading pixels.

### Needed

- photo library: built (@exact/photos)
- image transforms: built (`transformImage`)

### Checkpoints

- `bun packages/photos/test/android.mjs --emulator` passes on API 29, 33 and 34
  emulators.
- The reader drew an EXIF-turned JPEG at its stored size, squashed; fixed in
  `host/linux/src/image/jpeg.rs` (the header's size is the upright one on Android).
- On API 29 every copy failed `UnknownError`: with no scratch store a drive's
  `app:/` files went under Rust's `temp_dir`, `/data/local/tmp`, since the platform
  sets no `TMPDIR` there; the reader now sets it to the app's cache.
