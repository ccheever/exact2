# Calendar

An Exact calendar example with Contract views and a Rust data module. The coral
**+** button opens a two-by-two picker for **Event**, **Plan**, **Todo**, and
**Sticker**, each with its own form. Saving from this global picker keeps the
date sheet closed.

Tap anywhere on a date, including a placed event or sticker, to open its
full-width bottom sheet. Schedules and the sticker appear as rows there; tap a
row to edit it, or hold it to drag it to another date. Tap the dimmed backdrop or
drag the top handle down to dismiss the sheet;
a short pull snaps back. Sheets slide out before being removed, including after
saving or cancelling a form. The date and Todo sheets are capped at 360 points
and 42% of the screen, including their bottom safe area (with a 176-point minimum
cap on small viewports). Longer lists scroll inside the sheet.

Hold a schedule or Todo row, then drag it onto a date. The row morphs into a
calendar block over 300 ms while the source sheet slides below the screen; the
same contact continues dragging. A successful drop keeps the sheet closed.
Drop in the bottom **Cancel** zone to cancel without saving and restore the
original sheet. Releasing outside the calendar or cancelling the contact also
restores it. Hold near the top or just above the Cancel zone to scroll, or near
the left or right edge to change months. Extra scroll room during a drag lets you
reach the last week above the Cancel zone.

The landing preview uses the destination bar's size and lane. The calendar's
row refresh waits until the 300 ms landing finishes, and the preview stays until
the refreshed calendar is ready. Sheets open in 200 ms with ease-out and close
in 180 ms with ease-in; tapping a date or Todo row morphs its sheet into the form
and fades the content. Reduced motion skips these animations. Changes
are saved in the app's local SQLite store.

The theme button in the header opens Classic plus five wallpaper themes: Blush
Notebook, Paris Haze, Meadow Morning, Lavender Sky, and Linen Journal. A theme
changes the calendar colors and event presentation and remains selected after
restarting the app.

Run these commands from the repository root, using the Bun version pinned in
`package.json`:

```sh
export EXACT_APP_DIR="$PWD/examples/ios/calendar"
bun host/web/dev.mjs --app calendar
```

For an iOS simulator:

```sh
export EXACT_APP_DIR="$PWD/examples/ios/calendar"
bun host/apple/build.mjs --ios --sim "iPhone 17" calendar-apple --run
```

For a connected iPhone:

```sh
bun host/apple/build.mjs --device calendar-apple --run
```

The existing Apple build script handles signing and reports missing prerequisites.

## Schedule dates

- The initial month and today's marker use the device's local date. Sample
  schedules are created only after the device reports its clock.
- The header shows `Sep`, or `Sep, 2025` outside the current year. The date sheet
  shows `Mon, Aug 31`, appending the year only when it differs from the current one.
- Events are filled calendar bars. Plans are colored lines with a starting dot
  and ending arrow; a segment continuing into another week omits that endpoint.
- Start and end dates are inclusive, including a timed schedule ending at midnight.
  A September 30–October 2 schedule appears on all three dates.
- All-day schedules store civil dates. Timed schedules also store separate local
  start/end clock times; they are not converted through UTC when edited.
- Moving a schedule preserves its inclusive date span and its clock times. Moving
  September 25–30 to September 20 produces September 20–25.
- A multi-day schedule is clipped to each visible week and month. Overlapping
  schedules occupy separate lanes.
- Dates from January 1900 through December 2100 are supported. Recurrence, calendar
  service synchronization, notifications, and shared calendars are outside this
  example.

## Todos and stickers

- Todos have a title, color, and completion state, with no date. Open the Todo
  list from the header to edit or complete them. Unfinished items appear first;
  completed items are muted and struck through.
- Dragging a Todo onto the calendar creates a one-day, all-day Event and removes
  the Todo in one transaction. Cancelling keeps the Todo in its original list.
- Choose a Sticker and a date in its form. Each date holds one sticker at the
  cell's bottom right, with space reserved below its schedule bars. Tap the date
  again to change or remove its sticker. Choosing a replacement updates that
  date's existing sticker.
- Hold a sticker in the picker to drag it straight onto the calendar. Tap a
  placed image to open its date sheet, then hold the sticker row to move it,
  including through month-edge paging. Dropping onto an occupied date replaces
  its sticker; moving a placed sticker clears its original date in the same
  transaction. The Cancel zone restores the source without saving.
- Existing schedules and their saved-operation journal are retained when the
  database adds Plans, Todos, and stickers.

## Source

`app.contract` and `parts.contract` own the calendar, sheets, forms, and visual
feedback. `data/` owns calendar arithmetic, lane placement, validation, and durable
changes. `apple/` and `web/` use the repository's existing host build paths. The
small modules under `modules/` recognize input and report it through the existing
native-module API; all rendered UI remains Contract.

The visual reference is `done-universal`; its Event and Plan styles, undated
Todos, and Todo dragging inform the presentation. Forms, stickers, and local
persistence are implemented in this example.

`assets/stickers/` contains fifteen transparent PNGs: sunshine, coffee, cake,
heart, sparkle, flower, book, workout, travel, rest, bunny, daisy, moon, paris,
and picnic. The original ten were generated with the built-in `image_gen` tool
as compact, centered soft-clay and gouache illustrations, then resized to
512 × 512 with `sips`, preserving transparency. Five wallpapers live in
`assets/themes/`. The same tool generated
`assets/icon.png`: a minimal black calendar symbol on an opaque white
background. The 1024 × 1024 icon is declared through `app.json`'s standard `icons`
field; the Apple build generates the required iPhone and iPad icon sizes.

## Verify

```sh
cargo test -p calendar-data --lib --tests
cargo test -p calendar-apple --test calendar
cargo run -q -p contract -- build examples/ios/calendar/app.contract -o /tmp/calendar.plan
```

Drive the running app through `bun scripts/agent.mjs ios` (or `web`). Useful IDs
include `today`, `previous-month`, `next-month`, `add-schedule`, `picker-event`,
`picker-plan`, `picker-todo`, `picker-sticker`, `schedule-title`, `start-date`,
`end-date`, `all-day`, `save-schedule`, `delete-schedule`, `todos-button`,
`date-sheet-handle`, `date-popup-backdrop`, `agenda-list`, `theme-button`,
`theme-0` through `theme-5`, and `cancel-zone`.

In the browser, a landscape-to-portrait resize can clamp a month's vertical scroll
offset to the top; the displayed month is retained.
