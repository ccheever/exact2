# Paperback Settings — notes

## What's built
One `NavigationStack` with an inset-grouped `List` holding Account, Reading, Appearance, Library, Notifications, Storage, About and Reset sections.
- **State:** a single `@Observable SettingsStore` (`Sources/SettingsStore.swift`) writes each property to `UserDefaults` in `didSet`. On launch it reads every value back and validates it: unknown enum strings and out-of-range sizes or times fall back to the defaults.
- **Pickers** (Font, Theme, Sort By, and Line Spacing at accessibility text sizes) push a checkmark list, like the iPhone's Settings app. They don't pop back when you pick something; Settings doesn't either.
  - In the font list, each font name is drawn in its own font.
  - In the theme list, each row has a small swatch.
- **Preview paragraph:** updates live as the font, size (slider, 12–32 pt) and line spacing (segmented control) change. It also uses sepia ink when the theme is Sepia.
- **Theme:**
  - System, Light and Dark set the window's `overrideUserInterfaceStyle`. `preferredColorScheme(nil)` doesn't reliably go back to the system appearance.
  - Sepia forces light mode and adds a paper-coloured background, row colour and a brown accent.
- **Notifications:** the master toggle reveals the time picker and three toggles, with animation.
  - Turning it on asks for notification permission and schedules a repeating daily local notification at the chosen time.
  - If permission is denied, the footer says so and offers an "Open Settings" button.
- **Confirmation dialogs** on Sign Out, Clear Cache and Reset All Settings.
- **Account:**
  - Name and email fields work with the keyboard: Next/Done return keys, a Done button above the keyboard, and swiping the list down dismisses it.
  - An invalid email gets a warning in the section footer.
  - Signing out clears the account and shows a "Sign In" state.
- **Largest text sizes:** label/value rows stack. The account fields put the label above the field. The segmented control becomes a pushed list, because a segmented control can't grow with the text.

## Decisions and guesses
- **Reset All Settings** restores the reading, appearance, library and notification settings. It leaves the account and the cache alone; they're data, not settings, and the dialog says so.
- **Storage numbers** are simulated, because there are no real books. Downloads are fixed at 1.34 GB. The cache is a persisted number that Clear Cache sets to zero. After that, Clear Cache is disabled.
- **Rate Paperback** calls StoreKit's `requestReview`.
- **Privacy Policy and Terms of Service** are short placeholder documents in the app.
- **Defaults:** New York 18 pt, Normal spacing, System theme, Recently Read, reminder at 8:30 PM.

## What was hard / what I couldn't do
- There's no way to tap the simulator: no idb or cliclick, and AppleScript UI scripting timed out.
  - I checked the app by writing settings with `simctl spawn … defaults write`, relaunching, and taking screenshots.
  - I also added two debug launch arguments: `-scrollTo <section>` and `-open <font|lineSpacing|theme|sortOrder|privacy|terms>`.
  - That confirmed persistence across relaunch, the Sepia, Dark and System themes, the notification reveal, the pushed pickers, the legal screens, and the largest accessibility text size.
- **Not exercised by a real tap:** the confirmation dialogs, the slider, the text fields and the keyboard. They are standard SwiftUI controls.
- The notification permission alert stayed on screen across relaunches, and even after reinstalling. I had to kill SpringBoard to clear it.

## What would have made this faster
A way to tap and type on the simulator (idb, or an XCUITest runner) so interactions could be tested directly instead of through launch arguments.
