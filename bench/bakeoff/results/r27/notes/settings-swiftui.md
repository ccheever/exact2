# Paperback Settings — notes

## What's built
One `NavigationStack` + inset-grouped `List` (`Sources/SettingsView.swift`):
- **Account**: an initials avatar header, editable Name and Email fields (proper keyboard types, Next/Done
  return keys, a Done button above the keyboard, drag-to-dismiss), and Sign Out behind a confirmation dialog.
  Signing out shows a "Not Signed In" row with a Sign In button.
- **Reading**: a font size slider (12–32 pt), a font picker with five fonts (New York, San Francisco, Georgia,
  Palatino, Avenir Next, each shown in its own face), a Compact/Normal/Relaxed segmented control, and a live
  preview paragraph drawn in the chosen font, size and spacing.
- **Appearance**: a System/Light/Dark/Sepia theme picker, applied to the whole app. Sepia uses a light color
  scheme with paper-colored backgrounds, brown text in the preview and a brown tint.
- **Library**: a Sort By picker, Show Finished Books, and Download over Cellular (the footer explains the choice).
- **Notifications**: a master toggle. Turning it on animates in a Daily Reminder time picker and three toggles.
- **Storage**: Used, Downloaded Books and Cache sizes; Clear Cache asks first and is disabled once the cache is empty.
- **About**: the version from the bundle, Privacy Policy and Terms of Service screens, and Rate the App
  (StoreKit `requestReview`).
- **Reset All Settings**: asks first, then puts back the defaults for reading, appearance, library and notifications.

The pickers use `.navigationLink` style: they push a list with a checkmark, like iOS Settings.
Every value is stored with `@AppStorage`, so it survives relaunch. Reset removes the keys, and the stored
values fall back to their defaults.

## Decisions and guesses
- Reset leaves account details and the cache size alone, because those are data, not settings (iOS's own
  "Reset All Settings" works the same way). The alert says so.
- There's no real cache. Its size is a stored number that starts at 248.3 MB and drops to zero when cleared.
  Downloaded books are a fixed 1.34 GB.
- The preview uses exact point sizes, so the slider alone sets the reading size. The settings around it
  follow Dynamic Type.
- At accessibility text sizes:
  - the account fields and avatar stack vertically;
  - line spacing becomes a menu picker, because segmented controls don't grow with Dynamic Type.
- The notification toggles don't request permission or schedule real notifications. The spec has no reading
  screen to remind people about, and a permission prompt would get in the way of testing.

## Testing
I only had `simctl`, which can't tap. So I tested by writing UserDefaults values with `simctl spawn defaults
write`, relaunching, and checking screenshots. That covered persistence, the four themes, dark mode, the
revealed notification rows and the largest accessibility text size. For screenshots further down the list,
I added a `-scrollTo <section>` launch argument (it only applies when that argument is given). I never saw the
confirmation dialogs, the pushed picker screens or the keyboard on screen; that code uses standard SwiftUI APIs.

## What would have made this faster
A way to tap and scroll the simulator (idb, or XCUITest through the build script) to test the interactive
flows directly.
