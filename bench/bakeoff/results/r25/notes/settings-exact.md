# Paperback Settings: notes

Built with Exact: `app.contract` (the view and every setting), `app.tamber` (the reading
paper/ink colours for Light, Dark and Sepia), `app.test.contract` (9 tests). Installed on
simulator `CEDFB9C9-…` as `dev.bakeoff.settings.b6c2df1`, named "Settings".

## What's there
- One inset-grouped list: Account, Reading, Preview, Appearance, Library, Notifications,
  Storage, About, and Reset All Settings.
- **Account:** editable Name and Email fields. Sign Out asks first in a system alert, then the
  section shows "Not signed in" with a Sign In button.
- **Reading:** a font size slider (12–32 pt, with the value shown), a Font row that pushes a
  checkmark list of five fonts (each name drawn in its own font), and a Line Spacing
  segmented control. A Preview section re-renders the paragraph live in the chosen
  font, size, spacing and theme paper.
- **Appearance:** System, Light, Dark and Sepia as checkmark rows. Choosing one calls
  `setScheme`, so the whole app follows it. Sepia uses a light scheme with a sepia page in the
  preview. A `mount` task applies the saved theme again at launch.
- **Library:** Sort By pushes a checkmark list (as the iOS Settings app does), plus two switches.
- **Notifications:** a master switch. When it is on, the section shows a Daily Reminder time
  picker (UIDatePicker) and three more switches.
- **Storage:** Used, Downloaded Books and Cache sizes. Clear Cache asks first, then shows
  Cache as "Zero KB" and disables the button.
- **About:** Version, plus Privacy Policy and Terms of Service, which push simple text pages.
  Rate the App calls `openURL` with an App Store write-review URL.
- **Reset All Settings** asks first, then restores the defaults for reading, appearance,
  library and notifications. It leaves the account and the cache alone, and its footer says so.
- **Persistence:** every setting is a root `state … persist` (UserDefaults on iOS). The tests
  `reload` and check that each value survives.

## Verification
- `bun exact.mjs test web`: 9/9 pass.
- `verify` (web): tests pass, no findings in light or dark mode. The one remaining finding is
  at the largest text size, where the web's segmented-control labels clip.
- `verify ios`: tests pass, no findings in light or dark mode, and nothing spills at
  AX-XXXL. Scrolling was "like UIKit" on two runs. The last run flagged a "rubber band"
  metric with no measured value ("–") while the machine's load was around 800. That control is
  UIKit's own collection view, so I think the flag came from the measurement under load.

## What was hard / looked up / guessed
- **Radio `change` payload:** a radio's `change` passes its `value` string, not a bool. The
  compiler's message said so exactly.
- **Layout inside a grouped row:** a row with text + control + text didn't stretch the slider
  until I wrapped it in a flex `view`. A label stacked above a segmented control needed
  `row flex-direction="column"`. My first try, `padding="10px 0"`, wiped out the cell's
  side margins, so I switched to `padding-top`/`padding-bottom`.
- **Links:** with the root's `navigate=follow`, an external `link href` was routed into my
  own router and did nothing. I found `openURL` in contract-grammar.md and used a button.
- **Guesses:** the account details, storage sizes, version number and App Store id are
  invented placeholders. Sepia is a light scheme plus a sepia preview page, because there
  is no reading screen to tint. Reset doesn't touch the account or the cache.
- **Not done:** the "Sepia" theme doesn't tint the settings chrome itself. Real
  notification scheduling and permission prompts are out of scope, since the spec only asks
  for the settings.

## What would have made this faster
- A settings-screen pattern with a pushed checkmark picker, a slider row, a segmented row
  under a label, and destructive confirmations. Most of my time went into finding
  those row layouts.
- Saying in the `grouped-form` recipe that a radio in a grouped row becomes the checkmark
  picker (it is only in the big control table in contract-for-agents.md).
- `openURL` listed on the card. It is missing from the card's function list.
