# Paperback Settings: notes

Built with Exact (`app.contract`, the view; `app.tamber`, the two sepia colours; `app.test.contract`,
9 tests). Installed on simulator C2188CC6-… as `dev.bakeoff.settings.b03dcec` ("Settings").
`test web`, `test ios`, `verify` and `verify ios` all pass the tests. The two findings left are below.

## What's there
- One inset-grouped list with seven sections: Account, Reading, Appearance, Library, Notifications,
  Storage and About, then Reset All Settings.
- **Pickers:** Font (5 fonts: System, New York, Georgia, Palatino, Avenir Next), Theme (System,
  Light, Dark, Sepia) and Sort By each push a list of choices with a checkmark, as iOS Settings does.
  The current value shows on the row in grey, with a chevron. Each pushed screen has a "‹ Settings" Back button.
- **Line Spacing** is a segmented control. At iOS's accessibility text sizes it becomes a menu
  pop-up, because three segments can't hold their labels there.
- **Preview:** a "page" card whose font, size (slider, 12 to 32 pt), line spacing and theme update live.
  Sepia gives it warm paper and brown ink. Theme applies with `setScheme` (Sepia uses the light scheme)
  and is applied again at launch.
- **Account:** Name and Email are editable fields. Sign Out asks first; after it, a Sign In button appears.
- **Notifications:** the master switch shows or hides a daily reminder time and three more switches.
  The footer spells out the reminder time.
- **Storage:** Used, Downloaded Books and Cache sizes. Clear Cache asks first and is disabled once the cache is empty.
- **About:** Version, Privacy Policy and Terms of Service (simple pushed pages), and Rate the App
  (an alert whose button opens an App Store review URL).
- **Persistence:** every setting is a `persist` state (UserDefaults), tested across a `reload`.
- **Reset All Settings** asks first, then restores every default. It leaves the account and cache alone.

## What was hard
- **iOS cells dropped my styling.** The preview first ignored both its background and its
  `line-height` on iOS, though it was correct on the web. `layout` showed the styles were set but the
  node was `hidden … hiddenBy` the section. A list row whose content is only text is drawn as a
  native label cell, so a box's paint and line-height are lost. Adding a non-text child (a thin
  rule under a "Chapter One" heading) kept it a custom layout. I found this only by experiment; a lint
  or a doc line ("a row of text only becomes a UIKit content configuration") would have saved ~20 minutes.
- **Driving quirks:**
  - Back in tests is the Back button's text (`tap "Settings"`), not its id.
  - A slider drag needs `--touch platform` and `from x y` placed on the thumb.
  - A multi-word testId must be quoted inside the op.
- **Contract has no `const`.** Constant lists became zero-argument `fn`s. It refuses `parseInt` in
  favour of `parseNumber` with a `match`.
- A radio's `change` passes the radio's `value` (a string), not a bool. The compiler said so clearly.

## Guesses
- Picked the five fonts, four sort orders, the three notification kinds, the storage numbers,
  the version, and the Privacy/Terms text myself. "Sepia" uses the light system scheme with a sepia reading page.
- Rate the App opens a placeholder App Store URL (`id1234567890`). In the simulator that goes to Safari.

## Not finished / known findings
- `verify` (web) reports the segmented control clipping at the web's largest root font (53 px). The
  web can't tell when the person is at an accessibility text size, so the menu swap is iOS only.
  `verify ios` reports nothing spilling.
- `verify ios` has one scrolling finding (rubber band at the top). I set no `overscroll-behavior`,
  so by the report's own rule it is Exact's. The machine was also too loaded for timing to be judged.

## What would have made it faster
- A `settings` pattern: a grouped list with a pushed checkmark picker, a slider, a time row and
  confirm alerts. Most of this app is that.
- Documenting the text-only-row → native-cell behaviour next to "a row of your own parts".
