# Paperback Settings: build notes

Built with Exact (`app.contract`, `app.tamber` for the sepia paper colours, `app.test.contract`).
It is installed on simulator 486FD498-… as **Settings**, bundle id `dev.bakeoff.settings.3bc92a`.

## What's there
- One long inset-grouped list with Account, Reading, Preview, Appearance, Library, Notifications, Storage, About, and Reset All Settings at the bottom.
- **Account:** editable Name and Email fields. Sign Out is destructive and asks first in an alert; once signed out you see "Not signed in" and a Sign In button.
- **Reading:**
  - A font-size slider from 12 to 30 pt, showing its value.
  - Font pushes a checkmark list of five fonts (Georgia, Palatino, Charter, Avenir Next, San Francisco), each name drawn in its own font, with a "‹ Settings" back button.
  - A Compact / Normal / Relaxed segmented control.
  - A live preview card that follows the font, size, spacing and theme (light, dark or sepia paper).
- **Appearance:** System, Light, Dark and Sepia as checkmark rows. The app's scheme follows the choice right away and again at launch. Sepia keeps the UI light and shows sepia paper in the preview.
- **Library:** Sort Order pushes a checkmark list. There are switches for Show Finished Books and Download over Cellular.
- **Notifications:** the master switch reveals a Daily Reminder time picker (UIDatePicker) and three more switches. The footer text changes with it.
- **Storage:** Used space and Cache size. Clear Cache asks first, then shows "Zero KB" and disables itself.
- **About:** Version 1.0 (1), Privacy Policy and Terms of Service open pushed text screens, and Rate Paperback opens the App Store review URL.
- **Reset All Settings** asks first, then restores the reading, appearance, library and notification defaults. Account and cache are kept, as on iOS.
- **Persistence:** every setting is a `state … persist` (UserDefaults).
- **Checks:** 10 tests cover all of the above, including relaunch and reset. `verify` (web) and `verify ios` both pass. On iOS, `verify ios` was clean in light, dark and the largest text size.

## What was hard / guessed
- **The bundle id is refused.** Exact's app.json schema requires every id segment to start with a letter, so `dev.bakeoff.settings.3bc92a` fails (Apple allows it). I build as `dev.bakeoff.settings.s3bc92a`. Then `install-ios.sh` copies the built .app, stamps the real bundle id and marketing version 1.0 into Info.plist, re-signs it ad hoc and installs it with simctl. Re-run `bun exact.mjs ios && ./install-ios.sh` after a change. `bun exact.mjs ios` alone installs under the stand-in id.
- **The preview card's paint disappeared on iOS.** In a grouped list, a row whose only child is a flex column of one text is read as a standard cell, so its background and padding were dropped. I only found this by reading `kernel/src/grouped.rs`. The fix was a row-direction box inside a card-less (`background-color="transparent"`) section, plus `padding=0` on the row so the card lines up with the other cards.
- **Radios pass their `value` (a string), not a bool.** The compiler said so clearly.
- **Agent drives start at clock 0,** so the theme restored by the launch task only shows after `clock +50`. A real launch is fine.
- **Guesses:**
  - The font list, sort options, the 1,240 MB of books plus 312 MB of cache, and the account name.
  - That Sepia means a light UI with sepia reading pages.
  - That Reset keeps the account and the cache.
  - The App Store URL behind Rate is a placeholder id.

## Not finished / limits
- The version text is fixed ("1.0 (1)"). Exact gives no way to read the bundle version, so `install-ios.sh` sets Info.plist to match.
- Rate the App opens a placeholder App Store URL; there is no real app id.
- The web build's checker still notes a 2 px overflow on the preview paragraph at some font and spacing settings. iOS is clean.

## What would have made it faster
- An app.json schema that accepts any valid Apple bundle id, or a separate `host.ios.bundleId`.
- A documented rule for when a grouped row is "standard" and loses its authored paint (or a warning when that happens).
- A grouped-list "picker row" that pushes a checkmark list, since Settings screens use it so often.
