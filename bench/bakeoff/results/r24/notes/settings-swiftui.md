# Paperback Settings — notes

## What's built
One `NavigationStack` with a grouped `List`, laid out like iPhone Settings (coloured icon squares, trailing values, chevrons):

- **Account**: an avatar/name/email row that opens a Profile editor (name and email fields; an email only saves when it's valid). Sign Out asks you to confirm. Once you're signed out, a Sign In screen takes its place.
- **Reading**: a font size slider (12–32 pt) and a font list: New York, San Francisco, Georgia, Palatino and Avenir Next, each shown in its own typeface. Line spacing is a segmented control. At accessibility text sizes it becomes a pushed list instead, because the segment labels don't fit. A live preview paragraph has a footer that sums up the current settings.
- **Appearance**: the theme list (System / Light / Dark / Sepia) has small swatches. The theme is applied through `UIWindow.overrideUserInterfaceStyle`, because `preferredColorScheme(nil)` doesn't reliably go back to System. Sepia uses Light mode plus paper-coloured backgrounds on every list and screen.
- **Library**: Sort By list, Show Finished Books, Download over Cellular (its footer changes with the toggle).
- **Notifications**: the master toggle reveals, with animation, a Daily Reminder time (compact `DatePicker`) and three toggles. Turning it on asks for notification permission and schedules a real repeating local notification at the chosen time. If permission was denied, the footer says so and links to iOS Settings.
- **Storage**: shows used space with a bar split into downloads and cache. Clear Cache asks to confirm, then disables itself when the cache is empty.
- **About**: Version comes from the bundle. Privacy Policy and Terms of Service open simple text screens. Rate the App opens a star-rating sheet.
- **Reset All Settings** asks to confirm. It's disabled when everything is already at its defaults.

Pickers open a pushed list with a checkmark, like iPhone Settings. Selecting an option keeps you on that screen, as Settings does.

Every value is saved to `UserDefaults` in a `didSet` on an `@Observable` store, so it's written immediately and survives relaunch.

## Testing
I checked on the simulator: the default screen, every section, sepia, dark mode, notifications on, empty cache, a changed font/size/spacing, and the largest accessibility text size. Persistence was checked by writing values with `defaults write` and relaunching. The pushed lists, Profile and the Rate sheet were checked with a temporary launch-argument switch, which I've since removed.

**Not tested:** I had no way to tap in the simulator (only `simctl`), so I never ran an interaction from start to finish: confirmation dialogs, dragging the slider, the reminder time picker, the permission prompt. That code is plain standard SwiftUI, but it has not been run with real taps.

## Decisions I guessed at
- Reset All restores preferences only. It keeps the account, the cache and the downloads, because those are data rather than settings, and the reset footer says so.
- Storage figures are mock values (the app has no real library). Only the cache size changes, and it stays cleared after relaunch.
- Rate the App uses a custom sheet, since there's no App Store ID for a real review prompt.
- The brand accent is a warm orange, used for tint and toggles.

## What would have made this faster
A way to tap and scroll the simulator, for example `simctl io tap`, or XCUITest without an Xcode project. Right now any interaction can only be checked by adding debug hooks to the app.
