# The camera by a person (LLP 1133.008 §10 drive 8, §13)

What no tool can do here: grant the camera on a Mac, open a phone's camera,
and look. Each step says what to run and what you should see.

## macOS, the conformance suite with a grant

1. `bun packages/camera/test/apple.mjs macos --ask`
2. Allow the camera dialog the fixture app shows. The suite opens the Mac's
   built-in camera (never an iPhone's through Continuity Camera), takes a
   photo, hides and shows the viewfinder, and stops the track; every line
   reads PASS. The camera's green light goes out while the viewfinder is
   hidden and comes back when it is shown.
3. Two sessions on one camera: `bun host/apple/build.mjs photo-editor-apple --host --run`,
   open Take Photo in each session and take a photo in each: both
   viewfinders show the camera, and each session's editor gets its own photo.

## The photo editor, by hand

- Web: `bun host/web/dev.mjs --app photo-editor`, open the page in Chrome,
  Firefox and Safari; Take Photo, allow, shutter: the photo is in the editor
  and Rotate 90° turns it.
- macOS: `bun host/apple/build.mjs photo-editor-apple --run`; the same.
- iPhone (drive 8): `bun host/apple/build.mjs --device photo-editor-apple --run`.
  1. Take Photo, allow: the back camera fills the sheet, turned as you hold
     the phone. Take a photo of the room: the editor shows it upright, and
     Rotate 90° turns it.
  2. Flip: the front camera, mirrored as in the Camera app; its photo is not
     mirrored. Torch On lights the torch (back camera).
  3. Tap the viewfinder: focus and exposure move to that point. Pinch: zoom,
     below 1× to the ultra-wide where the phone has one.
  4. The shutter sound plays (unless the ring switch silences it where the
     law allows).
  5. While the camera shows, open Control Center, or take a call: the
     viewfinder freezes or goes black; close it, and it comes back.
  6. Leave the app and come back: the camera stops while away and returns.
  7. Swipe the sheet down: the green dot goes out. Take Photo shows the
     camera again with no dialog.
  8. Turn the camera off in Settings > Photo Editor: on return the editor
     says the camera is off, with Open Settings.
  9. The restart time (D11): with the viewfinder hidden and shown again,
     note how long the camera takes to return (the journal's
     `capture started` beside the view's `displayed true`).
- Android: the photo editor's port waits (`rules/DEFERRED.md`); the fixture
  app is the Android acceptance (§14 question 5). Build it as
  `packages/camera/test/android.mjs` does (it builds and deletes it; keep its
  app directory to install the APK), install it on a phone, and with
  `--bind media-capture --bind camera`: Open, allow the dialog: the back
  camera shows in the viewfinder; Shutter: the shot line names
  `app:/tmp/camera/1.jpg` with a size; the shutter sound plays; Toggle hides
  the viewfinder and the camera's indicator goes out, Toggle again brings it
  back; Front opens the front camera; denying in Settings reads `prompt` on
  return, since Android may still ask, and `denied` once Open's ask was
  refused without a dialog. Tap and pinch do nothing on Android yet (hosted views take a click
  only).
