# Photos by a person (LLP 1133.019 §11 drive 9)

What no tool can do here: grant the photo library on a Mac or a phone, open
a photo held only in iCloud, and look. These steps need the real
implementations (stage 2 on Apple, stage 3 on Android); until those land, an
app outside the agent answers `NotSupportedError` and shows neither Save nor
Recents' strip. Each step says what to run and what you should see.

## The photo editor under the agent, by hand

- macOS: `bun host/apple/build.mjs photo-editor-apple`, then
  `bun scripts/agent.mjs macos --app photo-editor --storage photos --permission photos=prompt "tap show-recents" "clock +100" "tap @4 allow" "clock settle" "screenshot recents.png"`:
  the strip shows the six agent photos, numbered 1 to 6, each with a white
  band at its top (photo 3, stored turned, is upright too).
- iOS simulator: the same with `ios --sim <udid>`, then `tap @4 limited`
  instead of `allow`: three photos, "The photos you chose" and Manage.
- `bun packages/photos/test/drives.mjs <macos|ios|web>` runs every drive.

## iPhone (drive 9, stage 2)

`bun host/apple/build.mjs --device photo-editor-apple --run`

1. Tap Show under "Recent photos". The system asks for access to your
   photos with three choices. Choose "Limit Access…", pick three photos:
   the strip shows those three, with "The photos you chose" and Manage.
2. Tap Manage: the system's picker shows your selection. Add a photo and
   remove one: the strip follows, and a photo you removed that the header
   showed goes blank.
3. Open a photo held only in iCloud (Settings > Photos > Optimize iPhone
   Storage on, an old photo): it downloads and opens in the editor.
4. Open an HDR photo, rotate it, and Save to Photos: the system asks to add
   once (if only adding was asked before). In Photos the saved photo shows
   as HDR, with the original's date and place.
5. Turn photo access off in Settings > Photo Editor: on return the strip is
   gone and the editor says it can't see your photos, with Open Settings.

## Android 14 phone, in the fixture app (stage 3)

1. Read photos: the system offers "Select photos" and "Allow all". Select
   two: two photos. Reselect from the app's Manage: the selection changes.
2. Leave the app for an hour or more and return: the read status is read
   again (a selection can expire).
3. A fixture with `device.photos-add` alone: save a photo, then reopen it
   from the app with no prompt; its location is kept.
4. A fixture with `device.photos` granted and `ACCESS_MEDIA_LOCATION`
   refused: copy another app's photo; the copy says `locationRedacted` true.
