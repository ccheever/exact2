# Paperback Settings: notes

Built with Exact (`app.contract`, the view and logic; `app.tamber`, two sepia colours; `app.ts`
is empty because there is no data). Installed on simulator CEDFB9C9… as `dev.bakeoff.settings.b40bbcc`.
`bun exact.mjs verify` and `verify ios` are both clean: 9 tests pass, nothing flagged in light,
dark or at the largest text size, and scrolling is "like UIKit".

## What's there
- **Account:** editable Name and Email fields. Sign Out asks first. Once signed out, the
  section shows "Not signed in" and a Sign In button.
- **Reading:** a font size slider (12–28 pt, with the value shown); Font, a pushed list of five
  fonts, each drawn in its own face, with a checkmark; Line Spacing, a segmented control
  (Compact, Normal, Relaxed = 1.2, 1.5, 1.8×); a live preview paragraph plus a caption below it.
- **Appearance:** Theme is a pushed list (System, Light, Dark, Sepia). It sets the app's
  colour scheme with `setScheme`. Sepia uses light mode and gives the preview a paper colour
  and brown ink.
- **Library:** Sort By is a pushed list. Show Finished Books and Download over Cellular are toggles.
- **Notifications:** a master switch. When it is on, it reveals a Daily Reminder time (the
  native compact time picker) and three toggles.
- **Storage:** Used, Downloaded Books and Cache. Clear Cache asks first, then sets the cache to
  Zero KB and greys out the button.
- **About:** Version; Privacy Policy and Terms of Service, which push simple text screens; Rate
  the App, which opens an alert.
- **Reset All Settings** asks first. It resets reading, appearance, library and notification
  settings. It keeps the account and the cleared cache, and says so in the footer.
- Every setting is a root `state … persist` (UserDefaults), and the theme is re-applied at
  launch. Tests check that settings survive a relaunch.

## What was hard / guessed
- "Pickers open … the way an iPhone settings screen would": I pushed a checkmark list for each
  picker, as iOS Settings does, rather than using inline menus.
- Fonts: the five are San Francisco, Georgia, Palatino, Charter and Avenir Next, all installed
  on iOS. The default sizes, spacing values, storage figures, notification toggle names and
  legal text are all made up.
- Rate the App: there is no real App Store listing, so it shows a "Enjoying Paperback?" alert
  instead of opening StoreKit.
- Things I had to fix after the first compile: `showModal(...)` is only valid inside an action,
  so a button uses `commandfor/command="show-modal"`; `after(0, …)` is refused and needs
  `after(1, …)`; the colour role is `-exact-tertiary-grouped-background`. A 3-child
  slider row didn't stretch the slider, so I split it into a label row plus a slider row.
- At the largest text size, verify flagged the value rows and the segmented control. I fixed
  them by wrapping the title and value (`flex-wrap`) and capping the segment label size.

## Not finished
- Nothing from the spec is missing. Rate the App does not reach a real store.

## What would have made it faster
- A `settings`/grouped-form pattern that includes a pushed-picker row with a checkmark
  list, a value row, and a slider row. I put these together from several recipes.
- A list of colour role names in the card. I had to grep the docs to find the tertiary
  grouped background.
