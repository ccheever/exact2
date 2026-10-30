# Notes — Paperback Settings (SwiftUI)

## What's built
One `Form` in a `NavigationStack` with the sections Account, Reading, Appearance, Library, Notifications, Storage, About and Reset.
- **Pickers:** Font and Sort By use `.navigationLink`, which pushes a checkmark list like the iOS Settings app does. Each font name is drawn in its own typeface. Theme is an inline checkmark list with swatches. Line spacing is a segmented control.
- **Preview:** the paragraph uses the chosen font, size and spacing at the exact point size, so Dynamic Type doesn't scale it.
- **Persistence:** every setting lives in an `@Observable` `AppSettings`, and each property's `didSet` writes to UserDefaults. On load, stored values are checked: an unknown enum value falls back to its default, and font size and reminder time are clamped to their valid ranges.
- **Themes:** System, Light and Dark set `overrideUserInterfaceStyle` on the window, because `preferredColorScheme(nil)` doesn't reliably go back to the system style. Sepia uses the light style plus a sepia background, sepia rows, a brown tint and sepia preview text.
- **Notifications:** turning on the master toggle reveals Daily Reminder, New Releases, Reading Goals and Recommendations with an animation. It also asks for notification permission (only when the user turns it on, never at launch) and schedules a real repeating daily reminder. If permission is denied, the footer explains this.
- **Confirmations:** Sign Out, Clear Cache and Reset All Settings each use a confirmation dialog (an action sheet).
  - Signing out switches the section to a Sign In button, which opens a sheet with validated fields.
  - Clear Cache sets the cache to zero, persists it, and disables the button.
  - Reset puts every preference back to its default. It leaves the account and the cache alone, as iOS's own "Reset All Settings" does.
- **Account:** the name and email fields edit in place, with an email keyboard, Next/Done submit buttons, a keyboard Done button and a validation footer.
- **About:** Privacy and Terms push simple text screens. Rate the App calls StoreKit's `requestReview`.

## What was hard or guessed
- A form-wide `foregroundStyle` (for sepia text) also overrode the red destructive buttons. I limited the sepia text color to the preview.
- `listRowBackground` has no effect when applied to the Form, so it has to be set on each section.
- `swiftc` with `-parse-as-library` didn't accept bare `/regex/` literals, so I used `#/.../#`.
- A notification-permission alert from an earlier run stayed on screen across app reinstalls until I rebooted the simulator.
- Guesses, since the spec doesn't say:
  - The five fonts: San Francisco, New York, Georgia, Palatino, Avenir Next.
  - The sort options.
  - The three extra notification toggles.
  - The storage figures, which are fixed numbers (842.7 MB of books plus a 356.4 MB cache).
  - The defaults.
  - That Reset leaves the account alone.

## Not verified
- I had no tool for tapping the simulator. I checked the screens by writing UserDefaults with `simctl spawn defaults write` and by a development-only `-scrollTo <section>` launch argument, then taking screenshots. I checked light, dark, sepia, notifications on, signed out, and the largest accessibility text size.
- The navigation pickers, confirmation dialogs and keyboard handling are standard SwiftUI, but I did not drive them by hand.
- Rate the App may show nothing on the simulator for an app not installed from the App Store.

## What would have made this faster
A way to tap and scroll the simulator (idb or an XCUITest runner) so interactions could be tested directly, not just checked through state injected into UserDefaults.
