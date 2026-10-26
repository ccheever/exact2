# Paperback Settings — notes

**What's built:** I built a single `NavigationStack` and `Form` (Sources/SettingsScreen.swift) with the sections Account, Reading, Appearance, Library, Notifications, Storage, About and Reset. Every value is stored with `@AppStorage` (keys and defaults are in `Sources/Model.swift`), so each one survives a relaunch. I checked this by writing defaults with `simctl spawn … defaults write` and relaunching.

- **Pickers:** Font and Sort By open a pushed checkmark list, like the iOS Settings app. I wrote my own `ChoiceList` so the list can take the sepia theme; the built-in `.navigationLink` picker style can't be styled. Theme uses inline checkmark rows with swatches. Line spacing is a segmented control and the reminder time is a compact `DatePicker`.
- **Preview:** It follows font, size (12–32 pt) and line spacing as they change. It also uses the page colours of the current theme.
- **Theme:** I set `overrideUserInterfaceStyle` on the windows instead of using `.preferredColorScheme`, so dialogs and alerts follow the theme too. Choosing System clears the override properly. Sepia uses a light interface with warm backgrounds and a brown tint.
- **Confirmations:** Sign Out, Clear Cache and Reset all ask first. Clear Cache is greyed out once the cache is at zero.
- **Sign out:** It shows a "Not Signed In" state with a Sign In sheet. Sign In stays disabled until the name is filled in and the email is valid.
- **Name and email:** You can edit them in place. Return moves to the next field, there is a keyboard Done button, scrolling dismisses the keyboard, and a red footer appears for an invalid email.
- **Large text:** I checked layout at AX5. The account header stacks vertically at accessibility sizes, and the storage legend wraps.

**Guesses I made:**
- **What Reset covers:** It restores reading, appearance, library and notification settings. It does not sign you out, edit your name or email, or bring back the cleared cache, because those are account data rather than preferences. The confirmation dialog says so.
- **Storage figures:** They are made up. Downloaded books are 1.18 GB, the cache starts at 248 MB, and the bar is drawn against a 4 GB quota.
- **Rate the App:** It calls StoreKit's `requestReview`, which may show nothing on the simulator.
- **The five fonts:** New York, San Francisco, Georgia, Palatino and Iowan Old Style, all of which ship with iOS.

**What was hard:** `simctl` has no way to tap or scroll. I added a testing-only `-scrollTo <section>` launch argument to screenshot the lower sections, and set state with `defaults write`. Because of that I could not drive these interactions on the simulator, and checked them only by reading the code: the confirmation dialogs, the pushed pickers and the Sign In sheet.

**What would have made it faster:** A way to tap and scroll from the command line, such as idb or an XCUITest runner.
