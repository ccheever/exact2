# Notes: Paperback Settings (Exact)

Built with Exact. `app.contract` holds the whole app, and `app.ts` answers no sources. Every
setting is a root `state … persist`, which is UserDefaults on iOS. `bun exact.mjs verify`
and `verify ios` both came back clean, all 9 tests pass, and the app is installed on simulator
CEDFB9C9-… as `dev.bakeoff.settings.b56bfa9` ("Settings").

## What's there
- **Account:** editable name and email fields in their rows. Sign Out asks first in an alert;
  once signed out, the section shows "Not signed in" and a Sign In button.
- **Reading:**
  - A font size slider from 12 to 32 pt, with small and large "A" buttons that step it by 1.
  - Five fonts (San Francisco, New York, Georgia, Palatino, Avenir Next), picked on a pushed
    checkmark list where each name is drawn in its own typeface.
  - Line spacing as a segmented control (Compact, Normal, Relaxed).
  - A live preview paragraph that follows the font, size and spacing.
- **Appearance:** System, Light, Dark or Sepia, picked on a pushed checkmark list. The choice
  goes through `setScheme`, and a `mount` task applies the saved one at launch. Sepia sets the
  light scheme and draws the preview on warm paper.
- **Library:** sort order (pushed checkmark list) and two switches.
- **Notifications:** a master switch. Turning it on reveals a daily reminder time (the
  platform's time picker) and three more switches, and the footer names the reminder time.
- **Storage:** Used, Downloaded Books and Cache sizes. Clear Cache asks first, then shows
  Cache as "Zero KB" and disables the button.
- **About:**
  - The version (1.0 (1)).
  - Privacy Policy and Terms of Service, each pushed as its own screen.
  - Rate the App, which opens an App Store review URL.
- **Reset All Settings** asks first, then restores every reading, appearance, library and
  notification default. It keeps the account and the downloads.

## What was hard
- **The preview's sepia background.** A computed `background-color` (a ternary) on a grouped
  list row, or on a column inside it, did not paint on iOS, though a computed `color` did. I
  had to split the preview into `if sepia … else …` branches with a literal colour on each.
- **Test IDs.** I gave a section the testId "notifications", and `tap "notifications"` hit
  the section instead of the switch. Section test IDs now start with `section-`.
- **Small layout fixes from verify:**
  - The largest text size clipped the segmented control's labels, so their font size is
    capped.
  - The value rows (Font, Theme, Sort Order) needed a wrapping inner row so the value can
    drop under the label.
  - The "A" buttons needed 44 pt minimum targets.

## What I looked up or guessed
- **Sign Out uses an alert, not an action sheet.** Its dialog has no message text, which the
  docs say should make an action sheet, but iOS drew a centred alert. I left it as an alert.
- **Picker style:** pushed checkmark lists, as iOS Settings does, rather than in-row menus.
- **Invented data:** the storage sizes (1.2 GB of books, 244 MB of cache), the account and
  the version number.
- **Rate the App** opens `https://apps.apple.com/app/id6450000000?action=write-review`,
  which is a made-up ID. A real app would use its own App Store ID or StoreKit's review
  prompt.
- **The reminder time** is stored, but no local notification is scheduled. The spec asks only
  for the setting.

## Not finished
- One `verify ios` run flagged the scroll feel (rubber band and momentum) while the machine
  was heavily loaded. The tool attributes those numbers to Exact itself, and the earlier and
  final runs were clean.

## What would have made this faster
- A grouped-settings pattern with value rows that push a checkmark chooser. That covers most
  of this app.
- A clear note on which style properties can be computed on iOS list rows.
