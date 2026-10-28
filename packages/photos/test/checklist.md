# Photos by a person (LLP 1133.019 §11 drive 9)

What no tool can do here: grant the photo library on a Mac or a phone,
answer the system's prompt, choose a limited selection, open a photo held
only in iCloud, and look. `apple.mjs` drives PhotoKit on an iOS simulator
(`--fill` adds the fixtures, `simctl privacy` grants) and only the status on
macOS; the steps below are the rest. Each says what to run and what you
should see.

## The photo editor under the agent, by hand

- macOS: `bun host/apple/build.mjs photo-editor-apple`, then
  `bun scripts/agent.mjs macos --app photo-editor --storage photos --permission photos=prompt "tap show-recents" "clock +100" "tap @4 allow" "clock settle" "screenshot recents.png"`:
  the strip shows the six agent photos, numbered 1 to 6, each with a white
  band at its top (photo 3, stored turned, is upright too).
- iOS simulator: the same with `ios --sim <udid>`, then `tap @4 limited`
  instead of `allow`: three photos, "The photos you chose" and Manage.
- `bun packages/photos/test/drives.mjs <macos|ios|web>` runs every drive.
- `bun packages/photos/test/apple.mjs ios --sim <udid> --fill` on a
  simulator you booted, then `apple.mjs macos`: PhotoKit bound.

## A Mac, by a person

`bun host/apple/build.mjs photo-editor-apple --bundle`, open the `.app`, tap
Show under "Recent photos": the system asks for access to your photos.
Write down which text it shows (D4 asks for it): the read key's, "Shows
your recent photos, so you can open one to edit.", is expected. Allow it,
and the strip shows your newest photos. Save an edit: it is added with no
second prompt. In System Settings > Privacy & Security > Photos, turn Photo Editor
off: on return the strip is gone and the denied line shows.

## iPhone (drive 9)

`bun host/apple/build.mjs --device photo-editor-apple --run`

1. Tap Show under "Recent photos". The system asks for access to your
   photos with three choices. Choose "Limit Access…", pick three photos:
   the strip shows those three, with "The photos you chose" and Manage.
2. Tap Manage: the system's picker shows your selection. Add a photo and
   remove one: the strip follows, and a photo you removed that the header
   showed goes blank.
3. Open a photo held only in iCloud (Settings > Photos > Optimize iPhone
   Storage on, an old photo): it downloads and opens in the editor.
   Age and Optimize Storage do not prove that an original is cloud-only.
   If uncertain, request its original through PhotoKit with network access
   disabled: no image data and `PHImageResultIsInCloudKey` true establish
   the prerequisite. Record unavailable coverage when every selected original
   is local; do not count opening a cached photo as an iCloud download.
4. Open an HDR photo, rotate it, and Save to Photos: the system asks to add
   once (if only adding was asked before). In Photos the saved photo shows
   as HDR, with the original's date and place.
   For a repeatable codec check, import the repository's
   `host/apple/tests/ExactKitTests/ColorFixtures/gainmap-iso.jpg`, whose HDR
   headroom is 4. Reopen the saved edit and inspect its original bytes with
   ImageIO using `kCGImageSourceDecodeToHDR`: `CGImageGetContentHeadroom`
   must remain 4 after a quarter-turn. A gain-map attachment alone is not
   enough; an SDR image can retain one. Use a fixture with known EXIF dates
   and GPS coordinates to check those fields as well.
5. Turn photo access off in Settings > Photo Editor: on return the strip is
   gone and the editor says it can't see your photos, with Open Settings.

## Android, in the fixture app (stage 3)

`apps/photos-fixture` is the library's consumer on Android (the photo editor
has no Android port). `bun packages/photos/test/android.mjs` builds its APK
and checks the manifest, the roster and the agent's photos in it, here, with
no device; `--emulator` adds the agent's drives (the door's six pictures, a
copy byte for byte, an add, limited access and a revocation), the codec's
conformance (step 9's checks among it) and MediaStore for real, bound,
granted by `pm grant` (the list, a copy with its location, an add no longer
pending) on an arm64 emulator (`ANDROID_SERIAL`) whose adb runs as root. It
passed on API 29, 33 and 34 emulators on 2026-10-28. The steps below are what
it cannot do: the platform's dialogs and what a person chooses in them.

Install it outside the agent with `bun scripts/agent-android.mjs build
photos-fixture --apk`, then `adb install -r
host/android/app/build/photos-fixture/outputs/apk/release/app-release.apk`.
Put photos on the device first: `adb push packages/photos/fixture/agent-3.jpg
/sdcard/Pictures/` and `adb shell content call --method scan_volume --uri
content://media --arg external_primary` (agent-3 is stored turned, EXIF
orientation 6).

### The permissions, on API 29, 33 and 34 (an emulator of each)

1. **API 29** (`READ_EXTERNAL_STORAGE`): the launch asks once, "Allow
   Photos Fixture to access photos and media". Allow: the status reads
   `granted full granted`, the grid shows the device's photos newest first,
   agent-3 upright. Record whether `ACCESS_MEDIA_LOCATION` showed a dialog
   of its own (D4).
2. **API 33** (`READ_MEDIA_IMAGES`): one dialog, "Allow … to access photos
   and videos"; the same grid. Deny twice: the status reads `denied none
   granted` and the launch asks nothing.
3. **API 34** (`READ_MEDIA_VISUAL_USER_SELECTED`): "Select photos" and
   "Allow all". Select two: `granted limited granted`, two photos and
   Manage. Tap Manage: the selection dialog again; choose one other: the grid
   follows, and the photo you left out goes blank (revoked) at once.
4. **API 34**, return after Settings > Apps > Photos Fixture > Photos and
   videos > Don't allow: the status and the grid change on resume, without
   relaunch.
5. Leave the app for an hour or more under a selection, and return: the
   read status is read again (a selection can expire).

### Copies, adds and the app's own photos

6. Tap a photo: `app:/tmp/photos/1.jpg image/jpeg WxH`. With
   `ACCESS_MEDIA_LOCATION` refused, copy another app's photo with a
   location: `adb shell run-as com.exact.photosfixture` and `exiftool` on the
   copy show no GPS, and the reply's `android.locationRedacted` is true;
   granted, the GPS is kept and it is false.
7. Tap Add the copy: a new photo under `Pictures/` (Files, Images), the
   grid's first; `adb shell content query --uri
   content://media/external/images/media --projection
   _display_name:is_pending:relative_path` shows it with `is_pending` 0.
8. A build whose grants are `device.photos-add` alone (edit
   `apps/photos-fixture/data/src/lib.rs`): no read dialog at launch; Add the
   copy of a photo already in `app:/tmp`, then the added photo's picture
   draws and copies with no prompt (the `owned` path, D6), with its location
   kept. Another app's id fails `NotFoundError`.

### The codec (D16): what the drive leaves

9. The orientation-6 checks (agent-3 upright at 360×480, its top half at
   360×240 and at 24×16) are the drive's. Left: an Ultra HDR JPEG from a
   phone's camera, turned by a quarter on an API 34 phone, keeps its gain map
   (`hdr` true); the drive's ISO 21496-1 fixture decodes with none there, so
   it says `hdr` false whatever the turn.
