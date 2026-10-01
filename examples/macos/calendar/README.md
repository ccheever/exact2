# Calendar for the Mac

The `examples/ios/calendar` app redesigned for a desktop window: the same
schedules, plans, todos, stickers and themes, with a Mac's layout and input.
Contract draws every pixel; a Rust data module owns dates, lanes and the local
SQLite store; a small native module reports mouse drags, trackpad swipes and
the Mac's context menu. The web build is the development loop.

| iOS | Mac |
|---|---|
| Date bottom sheet | An inspector beside the month shows the selected date |
| + button that opens a picker sheet | **Add** (⌘N) opens a centered dialog; Event, Plan and Todo switch inside it |
| Form sheets with drag handles | A modal dialog: Esc cancels, Enter in the title saves, the backdrop does not dismiss |
| Sticker sheet with its own date picker | A sticker palette in the inspector: click to use on the selected date, click again to remove, drag onto any date |
| Theme sheet | A theme panel under its button; a click outside or Esc closes it |
| Hold, then drag | Press and drag past three points, from an inspector row or the palette |
| Cancel zone | Esc, a drop outside the month, or a drop on the same date |
| Swipe between months | A trackpad swipe turns one month; ‹ Today › (⌘[ ⌘T ⌘]) too |
| Swipe to delete | Hover a row: an × at its right deletes the item |

## Run

Run these commands from the repository root, with the Bun version pinned in
`package.json`. The directory is named `calendar`, like the iOS example's, so
always name this app's crates (`macos-calendar-apple`, `macos-calendar-web`) or
pass `--app macos-calendar`; without a name the scripts pick the iOS crates.

```sh
export EXACT_APP_DIR="$PWD/examples/macos/calendar"
bun host/web/dev.mjs --app macos-calendar          # the web development loop
bun host/apple/build.mjs macos-calendar-apple --run # the Mac app
bun host/apple/build.mjs macos-calendar-apple --bundle --run  # Calendar.app with its icon
```

`--bundle` assembles `Calendar.app` and builds `AppIcon.icns` from
`assets/icon.png`, the 1024 × 1024 icon `app.json` declares (the iOS example's
icon). The plain `--run` binary has no bundle, so the Dock shows no app icon.

The window opens at 1180 × 800 and keeps at least 860 × 600. The themes are
light, so the app asks for the light appearance at launch: native date pickers
and switches never follow a dark system appearance into the light page.

## Use

- The month grid only selects dates. A click in a cell, on a bar or a sticker
  in it too, selects that date; a double-click adds an event on it. With a date
  focused, the arrow keys move the selection by a day or a week and cross into
  the next or previous month. The selected date shows an inset, rounded accent
  tint.
- A trackpad swipe left or right over the month turns exactly one month, however
  far it travels. ‹ Today › and ⌘[ ⌘T ⌘] do the same. The month switches at once
  and fades in. The header's ‹ Today › have no background, only a hover tint.
- Click an inspector row to edit it. Drags start only from inspector rows
  (events, plans, todos, the date's sticker) and palette stickers: press and
  move past three points. An event's color dot grows into the event block, its
  title moving on top; a plan's arrow grows into the plan; a sticker lifts as on
  iOS. The date you drop on becomes the item's first day, and the highlighted
  range shows the result before you let go. Hold at the month's left or right
  edge to change months while dragging, or near the top or bottom to scroll a
  tall month. Bars and placed stickers in the grid do not drag.
- Hover an inspector row to show an × at its right; it deletes the item. A
  todo's circle marks it done. On the Mac, a secondary click on a row opens
  Edit…, Mark as Done and Delete… (Remove Sticker on a sticker); every one of
  them is also reachable without the menu.
- The inspector shows or hides the sticker palette. The palette sticker that
  the selected date carries is outlined. Click another
  to replace it; click the outlined one again to remove it.
- Seven themes: Classic and six wallpapers (Blush Notebook, Paris Haze, Meadow
  Morning, Lavender Sky, Linen Journal, and San Francisco with the Golden Gate
  Bridge). A white gradient veil lies over a wallpaper, stronger toward the
  bottom, and plan names take the theme's ink there, so text stays legible on
  the busier lower half of each image. White text on every theme's accent
  (Add, today's date) is at least 4.5:1.
- ⌘N, ⌘T, ⌘[ and ⌘] also appear in the app's File and Go menus.

Dates run from January 1900 through December 2100. Recurrence, calendar sync,
notifications, dark mode and more than one window are outside this example.

## Source

- `app.contract`: the root component: its state, its actions and the window's
  layout, the inspector, the drag ghost, the theme panel and the dialog.
- `month.contract`: the month grid. It only selects: the Mac hit-tests in tree
  order, not by `z-index` or `pointer-events`, so a week lists its date cells,
  its bars, its stickers, then the date buttons above them all.
- `parts.contract`: small views: inspector rows and their delete buttons,
  palette stickers, theme choices, colors.
- `models.contract`: the records the data module answers and the input module
  posts, and the date, palette and key arithmetic.
- `data/`: a copy of the iOS example's data crate, kept apart from it, with its
  own app id (`com.exact.calendar.macos`, so its own store), `calendarItem` (a
  schedule read from memory, so a drag begins on its first packet) and each
  month's first and last day.
- `modules/apple/CalendarInput.swift`: AppKit views. A handle presses on a click
  and begins a drag past three points; it never calls the host's mouse handlers,
  so a click presses once. Esc and a lost key window cancel. A local scroll
  monitor turns one month per horizontal trackpad swipe and lets every scroll
  through, so the month still scrolls. The packets and their acknowledgement
  are the iOS module's.
- `modules/web/index.js`: the same on the web, with a wheel handler for the
  swipe. A mouse drags at once; a touch still holds for 300 ms, so the web
  agent's contacts (touches) need a `tap hold 400` before they move. The Mac's
  context menu has no web form.
- `assets/`: the iOS example's stickers and icon, and the theme wallpapers:
  landscape 2400 × 1600 images made for the desktop, not the iOS portrait ones.

## Verify

```sh
cargo test -p macos-calendar-data --lib --tests
cargo test -p macos-calendar-apple --test calendar
cargo run -q -p contract -- build examples/macos/calendar/app.contract -o "$TMPDIR/macos-calendar.plan"
```

The crates are workspace members but not default members: the async lane
(`--workspace`) tests them, not the blocking gate.

Drive the running app with `bun scripts/agent.mjs --app macos-calendar <web|macos>
--size 1180x800 --storage <name>`. On the agent's clock the library loads on the
first frames, so start with `clock +500`; a drag's packets wait for the
runner's acknowledgement, so give each step of a drag a few `clock` steps. On
the Mac, `screenshot <png> window` shows the window as composited; the plain
form leaves out images inside a scrolled month. Useful ids: `new-item`,
`today`, `month-previous`, `month-next`, `theme-button`, `theme-0`…`theme-6`,
`close-themes`, `date-<month id>-<date>` (`date-2026-09-2026-09-16`),
`bar-<month id>-<bar id>`, `sticker-<month id>-<date>`, `agenda-item-<id>`,
`delete-<id>`, `todo-check-<id>`, `sticker-palette-toggle`,
`sticker-pick-<sticker>`, `sticker-agenda-<date>`, `delete-sticker`,
`kind-event`, `kind-plan`, `kind-todo`, `schedule-title`, `start-date`,
`end-date`, `all-day`, `save-schedule`, `cancel-editor`, `delete-schedule`,
`confirm-delete`, `keep-schedule`, `calendar-input`.
