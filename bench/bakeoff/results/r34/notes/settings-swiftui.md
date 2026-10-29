# NOTES — Paperback Settings (SwiftUI)

## What's there
One `Form` in a `NavigationStack`, with these sections: Account, Reading, Appearance, Library, Notifications, Storage, About and Reset.
- **Persistence:** every setting is an `@AppStorage` key (`Sources/Model.swift`). Enums are `String` raw values.
- **Pickers:** Font, Theme and Sort By use `.navigationLink` pickers that push a checkmark list, as iOS Settings does. Each font
  name in the list is drawn in its own face. Line spacing is a segmented control and the reminder time is a compact `DatePicker`.
- **Preview** shows a paragraph in the chosen font, point size (fixed, 12–30 pt) and line spacing. It updates live and has a
  sepia page when the Sepia theme is on.
- **Theme:** I set `overrideUserInterfaceStyle` on the windows in UIKit, because `.preferredColorScheme(nil)` doesn't
  reliably go back to System. Sepia uses the light style plus warm list, row and tint colours.
- **Notifications:** the master toggle reveals the time and three toggles with an animation. Turning it on asks for permission
  and schedules a repeating daily reminder. If permission is denied, the footer links to iOS Settings. The app never asks for
  permission just because it launched.
- **Storage:** the cache is real. On first launch the app writes about 19 MB of placeholder files to Caches/Paperback.
  The app measures that folder from disk, and Clear Cache (with a confirmation) deletes it. The button is disabled when the
  cache is empty.
- **Account:** name and email can be edited inline. The email field uses the email keyboard, and the keyboard has Next/Done
  and a Done toolbar button. An invalid email shows a warning. Sign Out asks to confirm, then shows a signed-out state with a
  Sign In sheet.
- **Reset All Settings** asks to confirm, then removes every resettable key so the `@AppStorage` defaults come back. It also
  cancels the reminder. The account is deliberately left alone.
- **About:** the version comes from the bundle. Privacy and Terms push simple text screens. Rate the App uses `requestReview`.

## Checked on the simulator
I checked it in Light, Dark and Sepia, and at accessibility XXXL text. I set values through the app, relaunched, and they
survived. I seeded settings, ran Reset, and the defaults came back. I used temporary launch-argument hooks to scroll and to
trigger reset, and removed them afterwards. I couldn't tap anything, because simctl has no tap command. So I never actually
saw the pushed picker screens, the confirmation dialogs or the Sign In sheet, though they are standard SwiftUI.

## Hard or surprising
- `simctl spawn … defaults write <bundle id>` writes to a preferences domain that the app can read but can't delete from.
  So Reset looked broken until I seeded the values from inside the app.
- The notification permission alert got stuck in SpringBoard and showed on top of every app. Uninstalling didn't clear it;
  restarting SpringBoard (`launchctl kickstart -k`) did. That's why the app now asks only when the user flips the toggle.
- Swift's `/regex/` literal didn't parse in that position, so I used `#/…/#`.

## Guesses
I chose the five fonts (New York, San Francisco, Georgia, Palatino, Avenir Next), the sort orders, the three extra
notification toggles, the defaults, and the rule that Reset doesn't touch the account.

## What would have helped
A way to tap and swipe on the simulator (idb or XCUITest) to test the dialogs and the pushed screens.
