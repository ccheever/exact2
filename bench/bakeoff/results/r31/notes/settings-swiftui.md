# Notes: Paperback Settings (SwiftUI)

## What's built
One `Form` in a `NavigationStack`, with these sections: Account, Reading (with a live preview), Appearance, Library, Notifications, Storage, About and Reset.
- **Persistence:** an `@Observable SettingsStore` writes every property to `UserDefaults` in `didSet`. I chose this over `@AppStorage` so Reset can set the values directly and the UI updates at once. I checked by writing values with `defaults write`, relaunching, and taking screenshots.
- **Pickers:**
  - Font and Sort By use `.navigationLink` pickers: you push to a list with checkmarks, as in the iOS Settings app. Each font choice is shown in its own typeface.
  - Theme is an inline checkmark list with small swatches.
  - Line spacing is a segmented control.
  - Reminder time is a compact time picker.
- **Theme:** I set `overrideUserInterfaceStyle` on the window instead of using `preferredColorScheme`, because `nil` doesn't always switch back to "System". Sepia uses the light scheme plus a paper background, paper rows and a brown tint.
- **Confirmations:** Sign Out, Clear Cache and Reset use `confirmationDialog`s with destructive buttons.
  - Reset puts back the defaults for reading, appearance, library and notifications. It leaves the account and cache alone, and the dialog says so.
  - Clear Cache sets the cache to 0 (saved), and the button is then disabled.
- **Keyboard:** the email field uses the email keyboard. Return moves from Name to Email. There's a Done button above the keyboard, and scrolling dismisses the keyboard. A footer warns about an invalid email.
- **Largest text size:** the account header stacks vertically, and the avatar size is capped.

## Guesses
- The spec gives no details for these, so I made them up:
  - The five fonts: New York, San Francisco, Georgia, Palatino and Avenir Next.
  - Size range 12–32 pt (default 18) and spacing factors.
  - The storage numbers: downloads are a fixed 1.24 GB and the cache starts at 148.3 MB.
  - A sample name and email.
- After signing out, the screen shows "Not Signed In" and a Sign In button that restores the account. There is no real authentication.
- "Rate the App" opens an in-app star-rating sheet. The app isn't on the App Store, so `requestReview` or an App Store link wouldn't show anything useful.
- Privacy and Terms are short placeholder text screens.

## Not verified / limits
- I had no tool to tap the simulator. So I didn't check these by hand: the confirmation dialogs, pushing the picker screens, slider drags and the keyboard flow. They use standard SwiftUI controls.
- What I did check with screenshots: the layout in light, dark and sepia, the largest accessibility text size, settings surviving relaunch, and the revealed notification rows.
- There's a launch argument, `-scrollTo <section>`, that I added only to take screenshots of the lower sections. It does nothing in normal use.

## What would have made this faster
A way to tap and swipe in the simulator, or an XCUITest runner, so the interactive flows could be checked automatically.
