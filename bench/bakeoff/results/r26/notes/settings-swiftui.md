# Notes — Paperback Settings (SwiftUI)

## What's built
One grouped `Form` in a `NavigationStack`: Account (avatar header, editable name/email with an email
keyboard and validation footer, Sign Out with a confirmation dialog; signed-out state offers a Sign In
sheet), Reading (font-size slider 12–32 pt, font picker with San Francisco / New York / Georgia /
Palatino / Avenir Next, each shown in its own face, segmented line spacing, live preview paragraph),
Appearance (System / Light / Dark / Sepia), Library (sort order picker, two toggles), Notifications
(master toggle that animates in a daily-reminder time picker and three toggles), Storage (usage bar,
books / cache / total, Clear Cache with confirmation; disabled when empty), About (version from the
bundle, Privacy Policy and Terms of Service pushed screens, Rate Paperback via `requestReview`), and
Reset All Settings with a confirmation dialog.

Pickers use `.navigationLink` style, which pushes a checkmark list like iPhone Settings.
Every value lives in an `@Observable` `AppSettings` that writes to `UserDefaults` in `didSet`, so it
survives relaunch (verified by writing defaults, relaunching and screenshotting).

## Decisions / guesses
- Theme is applied through `window.overrideUserInterfaceStyle` rather than `preferredColorScheme`,
  because going back to `nil` (System) after forcing a scheme is unreliable in SwiftUI.
- Sepia = light mode plus a paper background, tinted rows, brown accent and sepia preview text.
- Reset All Settings restores preferences only; it does not sign out or refill the cache (like iOS's
  Reset All Settings, which leaves data alone). Signing out clears name and email.
- Storage numbers are simulated (no real library exists); the cleared cache size persists.
- The slider is continuous and rounds to whole points (a stepped Slider on iOS 26 draws a row of tick dots).
- At accessibility text sizes the account header stacks vertically and the avatar size is capped.
- A test hook: launch argument `-scrollTo library|notifications|storage|about` scrolls there on launch.
  I used it for screenshots because there's no tool here for tapping the simulator.

## Not verified
I couldn't tap anything in the simulator, so I never saw the confirmation dialogs, the pushed picker lists,
the sheet or the review prompt actually open. They're standard SwiftUI components. I did check
rendering in light, dark, sepia and AX5 text, plus persistence, using screenshots.

## What would have made it faster
A way to tap or scroll the simulator (idb or an XCUITest runner) so interactions could be tested end to end.
