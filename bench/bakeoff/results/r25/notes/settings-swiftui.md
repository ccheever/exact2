# Paperback Settings — notes

## What's there
- One `NavigationStack` + inset-grouped `List`: Account, Reading (+ live Preview), Appearance, Library,
  Notifications, Storage, About, Reset All Settings.
- Font, Theme and Sort By are `.navigationLink` pickers (push to a checkmark list, as in iOS Settings).
  Font options are drawn in their own typeface. Line spacing is a segmented control.
- Preview paragraph uses the chosen font at a fixed point size (the reader size is absolute, not Dynamic
  Type relative) with `lineSpacing` derived from the spacing option.
- Theme is applied with `UIWindow.overrideUserInterfaceStyle` instead of `preferredColorScheme`, because
  the latter doesn't reliably return to System and misses alerts/dialogs. Sepia = light style plus
  paper-coloured list background, rows and preview ink.
- Sign Out, Clear Cache and Reset All Settings each use a `confirmationDialog`. Signed-out state shows
  a Sign In row that opens a sheet (name + valid email required).
- Notifications master toggle animates in Daily Reminder (time picker) and three toggles.
- `SettingsStore` (`@Observable`) writes every property to `UserDefaults` in `didSet`; invalid or
  out-of-range stored values fall back/clamp to defaults.

## Decisions / guesses
- Reset All Settings resets preferences only; account and cache size are user data and are kept (the
  dialog says so).
- Storage figures are simulated (1.28 GB books + 347.5 MB cache, bar scaled to a 4 GB budget); clearing
  the cache persists 0 and disables the button.
- Rate the App calls StoreKit's `requestReview`; nothing visible may happen outside the App Store.
- Notifications don't request system permission or schedule real reminders (no reader to remind into);
  that would be the next step.

## Testing
No tap automation available, so I verified with `simctl` screenshots: `-scrollTo <section>` and
`-push privacy|terms` launch arguments jump to parts of the screen, and `simctl spawn defaults write`
seeded settings to check persistence across relaunch, sepia/dark themes and AX-XXXL text.
Interactive flows (dialogs, pickers, sign-in sheet, typing) were checked by code review only, not by tapping.

## What would have made it faster
A way to tap/scroll the simulator (e.g. idb or XCUITest runner) to drive dialogs and pickers.
