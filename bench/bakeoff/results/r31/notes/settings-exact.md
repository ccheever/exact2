# Paperback Settings: notes

Built with Exact, then installed on simulator `498D6D1F-…` as `dev.bakeoff.settings.b1f3b0f` ("Settings").
`bun exact.mjs verify` (web) and `verify ios` both pass clean: 10/10 tests, light and dark, the largest
text size with nothing spilling, and scrolling that matches UIKit.

## What's there
- One inset-grouped list with the sections Account, Reading, Appearance, Library, Notifications, Storage and About,
  then Reset All Settings at the bottom.
- The Font and Sort By pickers push a list with a checkmark, the way iOS Settings does. Theme is an inline
  checkmark list (four options). Line spacing is a segmented control and font size is a slider.
- The preview paragraph follows the font, size and line spacing live. In Sepia it sits on warm paper with brown ink.
  System, Light and Dark call `setScheme`, and Sepia uses the light scheme. A `mount` task applies the saved
  theme again on launch.
- Sign Out, Clear Cache, Rate the App and Reset all ask first with iOS alerts. Rate is followed by a "Thank You" alert.
  Clear Cache is disabled once the cache is empty.
- Turning Notifications on reveals the reminder time (the native time picker) and three toggles.
- Every setting is a root `state … persist`, which is UserDefaults on iOS. A real relaunch keeps them
  (tested by `reload`, and checked by hand on the simulator with dark theme).

## Hard, looked up, or guessed
- Most of the work was reading the docs. The patterns plus `contract vocab grouped-form persist choice confirm theme`
  covered almost everything. The app compiled after three small fixes: `after(0)` had to be `after(1)`,
  `aria-hidden` needed a bool, and `press=()` isn't allowed.
- Guessed content: the account (Jordan Lee), the storage sizes (books 1.2 GB, cache 248.6 MB), the version "1.0 (1)",
  the five fonts and the Privacy and Terms text. I chose fonts that iOS ships: Georgia, Charter, Iowan Old Style,
  Palatino and Avenir Next. Each has a CSS fallback for the web.
- Judgement calls:
  - Reset keeps the account and the cache, and its footer says so.
  - Sign Out shows a "Sign In" row that restores the account.
  - Rate the App can't reach a real App Store on the simulator, so it shows an alert flow instead.
- My first version used toasts. `verify` reported that they overlap rows, so I replaced them with alerts.
  At the largest text size the segmented labels were clipped, so I capped them with `font-size="min(-exact-subheadline, 24px)"`.

## Not finished
- Sepia only tints the preview, not the whole settings screen. On iOS the settings chrome stays in the platform's light scheme.
- Reminders don't schedule real notifications, and there's no permission prompt.

## What would have made it faster
- A built-in "settings picker" idiom: a row showing its value that pushes a checkmark list. I wrote it by hand from the navigation recipe.
- A way to scroll a grouped list in `agent ios` by id without a testId on the list. `tap` scrolling its target into view was enough here.
