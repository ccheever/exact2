# Settings (Paperback): notes

Built with Exact (`app.contract` + `app.tamber`; `app.ts` answers no sources). Installed on simulator
486FD498-0E3E-4B64-8E4B-152BD8265EE7 as `dev.bakeoff.settings.bd5e961` ("Settings").
`bun exact.mjs verify` and `verify ios` are both clean: 10 tests pass, light/dark, largest text size,
UIKit-like scrolling.

## What's there
- One inset-grouped list: Account, Reading, Appearance, Library, Notifications, Storage, About, Reset.
- Account: editable Name and Email fields. Sign Out asks first (an alert); once you're signed out, the
  section shows "Not signed in" and a Sign In button.
- Reading: a Font Size row (value in pt), a slider (12–32) with small and large "A" buttons, Font
  (pushes a list of five fonts, each shown in its own face), Line Spacing as a segmented control,
  and a live preview that follows font, size and spacing.
- Appearance: Theme pushes System, Light, Dark or Sepia. The first three set the app's scheme with
  `setScheme`, and a mount task re-applies it at launch. Sepia keeps the app light and puts the
  preview on warm paper with brown ink.
- Library: Sort Order (pushed list) plus two switches. Notifications: a master switch; when it's on,
  the Daily Reminder time picker and three more switches appear, and the footer names the time.
- Storage: Used, Downloaded Books and Cache. Clear Cache asks first, empties the cache, then disables itself.
- About: Version; Privacy Policy and Terms of Service push simple screens; Rate the App opens an
  alert offering 1–5 stars, and the chosen rating then shows on the row.
- Reset All Settings asks first, then restores every reading, appearance, library and notification
  default. It deliberately leaves the account and downloads alone, and its footer says so.
- Every value is a root `state … persist` (UserDefaults), so it survives a relaunch. A test covers
  this, and I checked it by hand on the simulator, including a kept Dark theme.

## What was hard
- **Backgrounds inside a grouped-list cell don't draw on iOS.** `background-color` on the row or on
  a nested column resolved correctly (`layout` showed `#f4ecd8`) but painted nothing. What worked
  was an absolutely positioned `view` behind the content, with the content lifted by
  `position="relative" z-index=1` and the row `position="relative" overflow="hidden"` so the web
  stays contained. This took several probe rebuilds to find.
- **`line-height` on the preview had no visible effect** in the first structure, where the column
  was the row's direct child. It did take effect in the positioned layout above, so both themes now
  use that one structure.
- Once, the Name/Email fields drew in the preview's serif font after an `if/else` swapped the
  preview's cell. That went away when I stopped swapping branches, so I couldn't isolate it.
- `after(0, …)` is refused, so the mount task uses `after(1, …)`. Drives on a virtual clock need
  `clock +N` before the restored scheme shows.

## Guesses
- "Pickers open as an iPhone settings screen would": I read this as a pushed list with a checkmark
  (Font, Theme, Sort Order), as in classic Settings, rather than a pop-up menu.
- Rate the App: there's no real App Store id, so an in-app star alert stands in for
  SKStoreReviewController.
- The storage figures are fixed sample values (1.2 GB of downloads, 249 MB of cache).
- Web verify flagged the segmented control's labels at a 53 px root font. A UISegmentedControl
  keeps its labels on one line, and `verify ios` finds nothing spilling, so `app.json` accepts
  these three web findings (`verify.accept`).

## What would have made this faster
- A note in the docs on what draws inside native grouped cells (backgrounds, line-height), or a
  compile warning when a background there will be dropped.
- A ready-made "pushed choice list with checkmark" pattern for settings pickers.
- A `scroll` op in the agent. I had to learn that scrolling is `tap <id> drag dx dy`.
