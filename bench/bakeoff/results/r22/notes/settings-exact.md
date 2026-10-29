# Paperback Settings: notes

Built with Exact. Bundle id `dev.bakeoff.settings.b2fc470`, name "Settings". It is installed on the
simulator `EEFC0029-…`. `bun exact.mjs verify` (web) and `verify ios` both pass all 9 tests.
On iOS, light and dark are clean and nothing spills at the largest text size.

## What's there
- One grouped list (`app.contract`) under a large "Settings" title. The sections are Account,
  Reading, Appearance, Library, Notifications, Storage, About, and Reset All Settings.
- The Font, Theme and Sort By pickers push a checkmark list, the way iOS Settings does. A
  "‹ Settings" back button and the edge swipe return to the list. Each font in the font list
  is drawn in its own face.
- The preview paragraph follows the font, the size slider (12–32 pt), the line spacing
  (a segmented control) and the theme. Sepia gives it paper and ink colours from `app.tamber`.
- The Theme setting applies the scheme to the whole app: System, Light or Dark. Sepia uses
  Light. A launch task re-applies the saved theme.
- Sign Out, Clear Cache, Rate the App and Reset All Settings each ask first in an alert.
  Clear Cache is disabled once the cache is empty. Rate opens the App Store URL.
- The Notifications master switch reveals the daily reminder time (a UIDatePicker) and three
  more switches.
- Every value is a root `state … persist` (UserDefaults on iOS), so all of them survive a
  relaunch. There is a test that relaunches and checks them.
- Reset puts back the reading, appearance, library and notification defaults. On purpose, it
  leaves the account, downloaded books and cache alone, as the section footer says.

## What was hard
- **A background inside a grouped-list row.** On iOS the colour was silently dropped in two
  cases: on the row itself, and on its single child box. It was only drawn once the box was
  nested two levels deep (`row > view > view background-color=…`).
- **I made up a colour role name** (`-exact-tertiary-system-grouped-background`, UIKit's
  spelling). The compiler didn't reject it; it just drew nothing. The real role is
  `-exact-tertiary-grouped-background`.
- **`line-height` as a bare number.** It is a CSS multiplier, even though the guide says a
  bare number is pixels. I gave it `size × factor`, which made a box about 6800 pt tall. Once
  the background showed, the text vanished. Fixed by passing just the factor.
- **`after(0, …)`** is refused; the soonest is `after(1, …)`.

## Guesses
- The made-up content: the account details, the 1.18 GB of downloaded books with a 214.6 MB
  cache, version "2.4.1 (318)", the five fonts (Charter, Georgia, Iowan Old Style, Palatino,
  San Francisco), the sort orders, and the three notification types.
- The pickers push a list and stay on it after a pick, as UIKit Settings does.
- Signing out is simulated: the section shows a "Sign In" button that restores the account.

## Not finished / known issues
- `verify ios` reported one scrolling finding: a tap that stops a gliding list also activates
  the row under it. Every row of mine uses `press=`, which the report says points to Exact
  itself. Its "blank samples" note likewise looks like the probe misreading plain rows.
- Web only, at the largest text size: the segmented control's labels clip, and the web alert's
  Cancel button overflows. iOS has neither problem.

## What would have made it faster
- A compile error for unknown `-exact-*` colour roles, and a warning when a background in a
  list row is dropped.
- A sentence in the docs that a bare `line-height` number is a multiplier.
- A settings-screen pattern with a pushed checkmark picker, since that's the most common
  iPhone settings idiom.
