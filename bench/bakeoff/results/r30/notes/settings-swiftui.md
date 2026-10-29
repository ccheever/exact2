# Paperback Settings — notes

## What's built
- `SettingsStore` (`@Observable`) holds every setting and writes it to `UserDefaults` in `didSet`, so every change persists right away. Bad or missing stored values fall back to the defaults, and the font size is clamped to its range.
- Sections:
  - **Account:** an avatar header, editable Name and Email fields (keyboard Next/Done, email keyboard, inline validation), and Sign Out with a confirmation dialog. Signing out shows a signed-out state with a Sign In button.
  - **Reading:** a font size slider (12–32 pt), a font picker (New York, San Francisco, Georgia, Palatino, Avenir Next, each name shown in its own face), a Compact/Normal/Relaxed segmented control, and a live preview paragraph that follows font, size, spacing and theme.
  - **Appearance:** a theme picker (System/Light/Dark/Sepia). Sepia forces light mode, uses paper-colored list backgrounds and a brown tint.
  - **Library:** a sort order picker and two toggles. The footer text changes with the cellular setting.
  - **Notifications:** a master toggle that animates in the reminder time and three toggles. Turning it on asks for notification permission. If permission is denied, the footer says so and offers Open Settings.
  - **Storage:** a usage bar, Downloaded Books and Cache sizes, and Clear Cache with a confirmation. The button is disabled once the cache is empty.
  - **About:** the version from the bundle, Privacy Policy and Terms of Service screens, and Rate Paperback (`requestReview`).
  - **Reset All Settings:** has a confirmation and is disabled when everything is already at its default.
- Pickers push a checkmark list like the iOS Settings app. The list stays open after a choice, as Settings does.
- Checked on the simulator: light, dark, sepia, the AX5 text size (the avatar row stacks and fields align left), and relaunching after settings were written to defaults.

## Guesses / decisions
- The spec doesn't say what Reset covers. Reset restores the reading, appearance, library and notification settings. It leaves the account and the cache, since those are data rather than settings, and the dialog says so.
- Storage sizes are simulated (no real library exists). Only the cache size changes, and it persists.
- The preview uses fixed point sizes because the slider sets an explicit reading size. All other UI scales with Dynamic Type.

## Not finished / limitations
- `simctl` cannot tap, so the dialogs, pushed picker screens and slider were not driven interactively on the simulator. I only checked them by building and reviewing the code. Screenshots covered layout, themes, text size and persistence; I set state by writing defaults and used a `-debugScrollTo <section>` launch argument (left in, and harmless) to reach the lower sections.
- No reminders are actually scheduled, because there is no content to remind about.

## What would have made it faster
- A way to tap and swipe on the simulator (an idb- or XCUITest-style driver) for checking the dialogs and navigation.
