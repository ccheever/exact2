# Calendar

One calendar for iOS, macOS and the web. The required root `app.contract` imports
`Calendar` from `src/app.contract`. The Contract in `src/` owns the shared
state, actions and interface; `data/` owns dates, schedules and local SQLite
storage. The platform folders contain build entrypoints and input modules.

## Run

From the repository root, using the Bun version in `package.json`:

```sh
bun install --frozen-lockfile
export EXACT_APP_DIR="$PWD/examples/calendar"
bun host/web/dev.mjs --app calendar
bun host/apple/build.mjs calendar-macos --run
bun host/apple/build.mjs --ios calendar-ios --run
```

Set `EXACT_APP_DIR` before using the repository build or agent commands, as with
other apps outside `apps/`. The common engine remains under
`host/apple`; the application's native code is under `ios/` and `macos/`.

## Layout and interaction

At widths of 860 pixels or more, the month has an inspector on the right and a
centered editor. Narrower windows use a date sheet, a floating add button with
Event, Plan, Todo and Stickers choices, and a bottom editor. The choice follows
window width on every platform. Resizing preserves the selected date, visible
month and editor draft; an active drag or sheet gesture is cancelled.

Select a date to inspect it. Edit an item from its row, mark a todo complete with
its circle, and delete from the editor or the row's hover button. Use the date
sheet's Close button or backdrop to dismiss it, or drag its handle to resize or
close it. The editor can also be dismissed using Cancel or its handle.

Drag an inspector item or palette sticker onto a date. In compact mode, items
and stickers in the grid can also be dragged. A touch holds for 300 ms first;
a mouse starts after moving three pixels. A cancelled drag makes no edit. Use
‹ Today ›, a horizontal trackpad swipe, or a compact calendar's horizontal touch
pan to move between months. Native context menus are available on macOS;
browsers may reserve Command/Control keyboard shortcuts, so visible controls
provide the same operations.

There are seven themes. Stickers and icons are shared; portrait and landscape
wallpapers are selected by layout. Dates range from January 1900 to December 2100.

## Sources

- `app.contract`: the fixed application entrypoint; imports the implementation.
- `src/app.contract`: shared resources, state, actions and responsive composition.
- `src/models.contract`: records, pure helpers and animations.
- `src/month.contract`, `agenda.contract`, `editor.contract`, `themes.contract`,
  `stickers.contract`: components grouped by feature, not by platform.
- `data/`: one data crate, including persistence and date/layout tests.
- `ios/`, `macos/`, `web/`: build entrypoints and each platform's `modules/`.
  The iOS entry reuses the native bake in `macos/build.rs`; UI and business logic
  do not live in either platform wrapper.
- `tests/`: shared baked-Contract behavior tests run against both native crates,
  including compact layout and breakpoint transitions.

## Storage

The app ID is `com.exact.calendar`. Existing iOS calendar data keeps its identity
and schema. Data from the former `com.exact.calendar.macos` and
`com.exact.calendar.web` examples is neither imported nor deleted. Browser data
also depends on its profile and origin. Sharing source does not synchronize
calendars between devices. Calendar sync, recurrence and notifications are out
of scope.

## Verify

```sh
export EXACT_APP_DIR="$PWD/examples/calendar"
cargo test -p calendar-data -p calendar-ios -p calendar-macos --lib --tests --no-fail-fast
bun host/web/build.mjs calendar-web
bun scripts/agent.mjs web --app calendar --size 1180x800 \
  --storage calendar-check --epoch 2026-09-16T12:00:00Z --time-zone UTC \
  "clock +500" "tap new-item" "clock +200" \
  "type schedule-title Shared calendar" "tap save-schedule" "clock +500" state logs
```

For a compact drive, use `--size 402x874` and insert `"tap add-event"` after
`"tap new-item"`. To inspect native builds, use `macos` or `ios` instead of `web`.
