# Paperback Settings: notes

## What it is
One inset-grouped settings screen (`app.contract`) with the sections Account, Reading,
Appearance, Library, Notifications, Storage, About and Reset All Settings. It pushes three
picker screens (Font, Theme, Sort By) and two documents (Privacy Policy, Terms of Service).

- Every setting is a root `state … persist`, kept in UserDefaults. `app.ts` has no sources.
  The tests relaunch with `reload` and check that the settings are still there.
- The pickers work the way iOS Settings does: the row shows its value in grey with a chevron,
  and tapping it pushes a list with a checkmark. You stay on that list and go back with
  "‹ Settings". The font list draws each name in its own face.
- Line spacing is a segmented control. At the accessibility text sizes
  (`exactViewport().accessibilityTextSize`) it turns into checkmark rows, because the
  three segments clip their labels there.
- The preview paragraph follows the font, size (12–32 pt slider), spacing and theme. Sepia,
  Light and Dark paint the preview's page from `app.tamber` tokens.
- The theme also sets the app's colour scheme with `setScheme` (Sepia uses light). A
  `mount` task applies the saved theme again at launch.
- The notifications master switch shows or hides the reminder time (`input type="time"`)
  and three more switches.
- Sign Out, Clear Cache, Rate the App and Reset All Settings each open an alert first.
  Sign Out and Reset are destructive.
  - After Sign Out, the section offers Sign In.
  - Clear Cache drops the cache to 0 MB, updates the Used total and disables itself.
  - Reset restores the reading, appearance, library and notification defaults. It leaves
    the account and the cache alone, and its footer says so.

## Guesses
- The spec leaves some content open, so I chose it myself: the five fonts (New York,
  Georgia, Palatino, Avenir Next, San Francisco), the sort orders, the three notification
  switches, the storage figures (1.2 GB of books plus a 284 MB cache), the version number and
  the policy text.
- "Rate the App" has no real App Store id, so it asks with an alert and then thanks the
  reader in the section footer.
- Whether Reset should also sign you back in or refill the cache. I decided those aren't
  settings, so it doesn't.

## What was hard / looked up
- Section titles: a `text role="heading"` inside a section draws as a plain row. A
  `header` / `text` child is what makes a grouped section's title. I only found this from the
  first screenshot.
- A pushed route's back button is tapped by its text ("Settings") in tests, not by
  `back`, because every route stays mounted.
- A footer can't sit inside an `if`; it has to be a conditional text.
- The largest-text check found the segmented control clipping, which led to the
  `accessibilityTextSize` fallback described above.

## Status
- The web tests pass (10/10).
- The web `verify` came back clean apart from the segmented-control finding, which led to
  the fallback above. The `A` labels beside the slider now use rem sizes, which fixed the
  fixed-type finding.
- iOS: I checked the screens by hand on the simulator, light and dark, along with the
  pickers and the preview.
- See the end of this file for the final `verify ios` result.

## What would have made it faster
- A settings-screen pattern (`pattern settings`) with section headers, value rows that push a
  checkmark picker, and a confirm alert.
- The grouped-form recipe could show a section `header` at the top.

## Final verify ios
`bun exact.mjs verify ios` was clean:
- the 10 tests passed;
- light and dark had no findings;
- nothing spilled at the largest accessibility text size;
- scrolling was "like UIKit".

The app (`dev.bakeoff.settings.b6ab80b`, "Settings") is installed on simulator
486FD498-0E3E-4B64-8E4B-152BD8265EE7. While I worked, the disk filled up once and blocked
the shell for a few minutes, then cleared up on its own.
