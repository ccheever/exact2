# Paperback Settings — notes

SwiftUI app, four source files:
- `Settings.swift`: the option enums and an `@Observable SettingsStore` that writes each setting to UserDefaults in `didSet`.
- `SettingsView.swift`: the main grouped list.
- `Components.swift`: the Settings-style option picker, preview paragraph, theme swatch, storage bar and avatar.
- `AccountViews.swift`: account edit, sign-in sheet, Privacy/Terms screens and notification scheduling.

## What's built
- **Account:** a profile row (avatar initials, name, email) that pushes an edit screen. Email is checked as you type and saved only once it's valid. Sign Out asks for confirmation, then the section shows a signed-out state with a Sign In sheet.
- **Reading:**
  - Font size slider (12–32 pt, whole points).
  - Font picker with five fonts: New York, San Francisco, Georgia, Palatino, Avenir Next. Each name is drawn in its own font.
  - Line spacing as a segmented control.
  - A live preview of a book's opening on a "page" coloured to match the theme.
- **Appearance:** System, Light, Dark and Sepia as an inline checkmark list. The theme is applied with `window.overrideUserInterfaceStyle`, so sheets and dialogs follow it too. Sepia gives the whole screen beige rows and background, a brown accent and a sepia preview page.
- **Library:** sort order picker and two toggles. The footer text changes with the cellular toggle.
- **Notifications:** turning on the master toggle reveals the daily reminder time and three toggles, with animation. It asks for notification permission, schedules a repeating daily reminder, and if permission is denied shows a footer linking to iPhone Settings.
- **Storage:** a used-space bar plus the sizes of downloaded books and the cache. Clear Cache asks first, the cleared value persists, and the button greys out at Zero KB.
- **About:** version taken from the app bundle, Privacy Policy and Terms of Service screens, and Rate Paperback using StoreKit's `requestReview`.
- **Reset All Settings:** asks for confirmation, then restores every reading, appearance, library and notification default. I treated the account and the cache as data rather than settings, so reset leaves them alone, and the confirmation message says so.
- **Pickers:** Font and Sort By push a checkmark list, as the iPhone Settings app does. Theme is inline, and line spacing is segmented.

## Tested on the simulator
I checked these with screenshots:
- light, dark and sepia themes
- different font, size and spacing settings reaching the preview
- the notifications section revealing its extra rows
- the Sign Out and Reset confirmations, which show as iOS 26 popover-style dialogs from their buttons
- the largest accessibility text size

## What was hard / guesses
- **No way to tap in the simulator.** I added launch-argument hooks (`-debugScrollTo <section>`, `-debugDialog reset|signOut|clearCache|signIn`) and passed settings as UserDefaults arguments (e.g. `-theme sepia`). Those arguments arrive as strings, which is why the store reads values with UserDefaults' typed getters.
- **The notification permission alert stays on screen across app relaunches and reinstalls.** Only restarting the simulator cleared it. `simctl privacy grant notifications` isn't supported.
- **Dialogs need to stay on their buttons.** Attaching a `confirmationDialog` to a row that the lazy list hasn't drawn means it never opens. Moving the dialogs onto the List avoided that but anchored them in the wrong place on iOS 26, so they stay on their buttons.
- **Guessed details:** the account name and email, the storage sizes, the policy text, and that Reset doesn't sign you back in or restore the cache.

## Not finished / caveats
- The disk filled up (ENOSPC) partway through. Once space freed I checked the stacked account header and the Reading section at the largest text size; both are fine.
- I didn't screenshot the expanded notifications section or the pushed screens (account edit, pickers, documents) at the largest text size.
- Persistence follows from the store writing every change straight to UserDefaults. Without a way to tap, I checked it only by launching with stored values, not by changing a setting and relaunching.

## What would have made this faster
- A way to tap and type in the simulator, or an XCUITest runner, so persistence and interactions could be checked end to end.
- A way to accept system permission alerts.
