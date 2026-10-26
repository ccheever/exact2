# Paperback Settings — notes

## What's there
- One grouped `List` with the sections Account, Reading (+ live Preview), Appearance, Library, Notifications, Storage, About, then Reset All Settings.
- Every value is stored with `@AppStorage`. The keys are in `Sources/Model.swift`. Reset removes the `Keys.resettable` keys from `UserDefaults`, and every view's `@AppStorage` goes back to its default. Reset deliberately leaves alone the account (name, email, signed-in state) and the cache size, because those are data rather than settings. The reset dialog says so.
- Pickers work the way iOS Settings does. Font and Sort Order push a checkmark list. The selected value shows in grey on the row, and the font list shows each name in its own typeface. Theme is an inline checkmark list with swatches. Line spacing is a segmented control.
- The theme is applied with `overrideUserInterfaceStyle` on the window, not `.preferredColorScheme`. That way, picking System again reliably follows the device, and confirmation dialogs follow the theme too. Sepia means light mode plus warm backgrounds and rows, a brown tint and sepia text in the preview.
- Sign Out, Clear Cache and Reset each ask for confirmation in a dialog. Signing out shows a "Not Signed In" state with a Sign In button. Clearing the cache sets it to 0 and disables the button.
- When notifications are turned on, the reminder time and three toggles appear with an animation. Turning them on also asks for notification permission and schedules a repeating daily local notification at the chosen time.
- Rate the App uses StoreKit's `requestReview`.

## Tested on the simulator
I tested by writing values into the app's defaults with `simctl spawn … defaults write` and relaunching. That covered persistence, the Sepia theme, the signed-out state and the revealed notification rows. I also checked dark mode and accessibility-XXXL text. For screenshots, the app has a `-scrollTo <section>` launch argument (`shot.sh`) so it can open already scrolled to a section.

## Not tested, and guesses
- simctl has no way to tap, so I did not test the interactive flows on screen: the confirmation dialogs, pushing the picker lists, Privacy and Terms, the Profile editor, and Reset. They use standard SwiftUI components. That Reset works depends on `@AppStorage` noticing `removeObject`, which it does, but I did not watch it happen here.
- The storage figures (1.42 GB of books, 248 MB of cache) are made up, because there is no data.
- At the largest text sizes the icons stop growing at xxxLarge, as in iOS Settings. The account row stacks vertically, and the email shrinks to fit on one line instead of breaking with hyphens.

## What would have made this faster
A way to tap and scroll in the simulator (for example idb or XCUITest) to test the dialogs and navigation from start to finish.
