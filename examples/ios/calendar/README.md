# Calendar

An Exact calendar example with one Contract view and a Rust data module. Tap a date
for its schedules in a full-width bottom sheet; tap a schedule to edit it, or use
the coral **+** button to add one. Tap the dimmed backdrop or drag the top handle
down to dismiss the sheet; a short pull snaps back. Its overall height, including
the bottom safe area, is capped at 360 points and 42% of the screen (with a
176-point minimum cap on small viewports). Longer lists scroll inside the sheet.

Hold a schedule row for a moment, then drag it onto another date. The entire sheet
moves below the screen and its backdrop disappears while a Contract preview
follows your finger. The sheet returns when the drop or cancellation finishes.
Hold near the top or bottom to scroll, or
near the left or right edge to change months. Changes are saved in the app's local
SQLite store.

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

## Source

`app.contract` owns the calendar, popup, editor, and visual feedback. `data/` owns
calendar arithmetic, lane placement, validation, and durable changes. `apple/` and
`web/` use the repository's existing host build paths. The small modules under
`modules/` recognize input and report it through the existing native-module API;
the popup, floating preview, date highlights, and editor remain Contract UI.

The visual reference is `done-universal`; its calendar and Todo dragging inform
the presentation. Schedule editing and persistence are new
in this example.

## Verify

```sh
cargo test -p calendar-data --lib --tests
cargo test -p calendar-apple --test calendar
cargo run -q -p contract -- build examples/ios/calendar/app.contract -o /tmp/calendar.plan
```

Drive the running app through `bun scripts/agent.mjs ios` (or `web`). Useful IDs
include `today`, `previous-month`, `next-month`, `add-schedule`, `schedule-title`,
`start-date`, `end-date`, `all-day`, `save-schedule`, `delete-schedule`,
`date-sheet-handle`, `date-popup-backdrop`, and `agenda-list`.

In the browser, a landscape-to-portrait resize can clamp a month's vertical scroll
offset to the top; the displayed month is retained.
