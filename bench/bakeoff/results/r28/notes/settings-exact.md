# Settings (Paperback): build notes

Built with Exact and installed on simulator `C2188CC6-…` as `dev.bakeoff.settings.b64569a` ("Settings").
`bun exact.mjs verify` (web) and `verify ios` are both clean: 8 tests pass, there are no light or dark findings, nothing spills at the largest text size, and scrolling is "like UIKit".

## What's there
- One inset-grouped list: Account (name and email fields, Sign Out with an alert, and Sign In afterwards), Reading (a font size value row and slider, a Font row that pushes a picker, a Line Spacing segmented control, a live preview), Appearance (System/Light/Dark/Sepia as checkmark rows), Library (a Sort Order row that pushes a picker, two switches), Notifications (a master switch that reveals a Daily Reminder time picker and three switches), Storage (used, downloads and cache, plus Clear Cache with an alert; the button is disabled once the cache is empty), About (version, Privacy Policy and Terms of Service on pushed pages, and Rate the App as an alert that opens the App Store), and Reset All Settings with an alert.
- Pickers work the way they do in iOS Settings. A row shows the current value and a chevron, and tapping it pushes a list where a checkmark marks the choice. The Back button is titled "Settings". Each font is listed in its own typeface.
- The preview follows the font family, the size in points and the line height (1.2, 1.5 or 1.9) as they change. With Sepia it draws on paper-coloured ground with brown ink, which come from tokens in `app.tamber`.
- Theme calls `setScheme`, so the bars, alerts and lists follow it. Sepia uses the light scheme plus the sepia preview. A `mount` task restores the saved theme at launch.
- Every setting is a root `state … persist`, which is UserDefaults on iOS. Reset restores the reading, appearance, library and notification defaults. It deliberately leaves the account and the cache alone, and the footer says so.

## What was hard or guessed
- **Painting inside a grouped row.** A `row` whose only child is a `column` of two texts was drawn as a native title and subtitle cell. My background colour, padding and even `line-height` were silently dropped: the preview ignored spacing on iOS while passing on web. I found the cause with `layout preview`, which showed the styles as authored but the box hidden. The fix was another `column flex=1` wrapper, following the hint in agent-pitfalls ("put the content in a column flex=1"). A compiler warning for "styles on a row that becomes a native cell" would have saved the most time.
- A slider beside two "A" glyphs in a row got only half the width until it was wrapped in a `row flex=1`. This is the same native-cell inference.
- Handler arity for radios: `change=choose(id)` also receives the radio's value. I switched to `value=id change=choose`, and the compiler's message said so clearly.
- `path()` takes only a literal route name, so I wrote one `open…` action per route.
- `-exact-tertiary-system-grouped-background` doesn't exist; the role is `-exact-tertiary-grouped-background`. I found it by grepping the docs, because `contract vocab` didn't list the colour roles.
- The segmented control clipped at the largest text size. I capped it with `font-size="min(-exact-subheadline, 22px)"`, as verify suggested; UIKit's segmented control also stops growing.
- These were guesses:
  - The five fonts (Georgia, Palatino, Times New Roman, Helvetica Neue, Avenir Next) and the font size range (12–32 pt, default 18).
  - The sort options (Recently Read, Title, Author, Date Added).
  - The names of the three notification toggles (New Releases, Reading Goals, Recommendations) and the 9:00 PM default.
  - The storage figures: 1.2 GB of downloads (fixed) and a 248.6 MB cache.
  - Rate the App as an alert that opens an App Store URL, since there is no real App Store id.
- The saved theme is applied by `after(1, …)` in a `mount` task, so on launch there may be one frame in the system scheme before a saved Light, Dark or Sepia applies. I found no way to set the scheme before the first frame.

## What would have made it faster
- A list of colour roles in `contract vocab`.
- A warning when a styled `column` inside a grouped `row` is turned into a native cell and its styles are dropped.
- `prefer` naming text sizes, not just `root-font-size`.
