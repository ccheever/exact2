# Recipe: dates and days

The date is `time.epochAtZero + performanceNow()`, with the reserved `exactTime()`
source. A day is a key, `"2026-03-14"`, in the person's own time zone.

```contract
shape Time
  epochAtZero: number
  utcOffset: number

component Habit
  state now = 0
  state doneDays = []
  state lastAt = 0
  resource time = exactTime() as shape Time
  derive shown = time.epochAtZero + now
  derive today = formatDate(shown, time.utcOffset, "iso")
  derive yesterday = formatDate(shown - 86400000, time.utcOffset, "iso")
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
  action markDone
    let at = time.epochAtZero + performanceNow()
    now = performanceNow()
    doneDays = doneDays.concat([formatDate(at, time.utcOffset, "iso")])
    lastAt = at
  view
    column padding=16 gap=8
      text formatDate(shown, time.utcOffset, "medium") role="heading" aria-level=2 testId="today"
      if doneDays.includes(today)
        text "Done today" testId="status"
      else if doneDays.includes(yesterday)
        text "Done yesterday" testId="status"
      else
        text "Not done yet" testId="status"
      if lastAt > 0
        text `Last done ${formatDay(lastAt, shown, time.utcOffset, "day-time")}` testId="last"
      button press=markDone disabled=doneDays.includes(today) buttonStyle="filled" testId="done"
        text "Mark Done"
```

```contract-test
test "the day turns over at local midnight"
  epoch "2026-03-15T06:30:00Z"
  time-zone "America/Los_Angeles"
  expect text "today" == "Mar 14, 2026"
  tap "done"
  expect text "status" == "Done today"
  clock +3600000
  expect text "today" == "Mar 15, 2026"
  expect text "status" == "Done yesterday"
  expect text "last" == "Last done Yesterday at 11:30 PM"

test "an entry made 30 s after midnight, before the tick, is the new day's"
  epoch "2026-03-15T06:59:50Z"
  time-zone "America/Los_Angeles"
  clock +40000
  expect text "today" == "Mar 14, 2026"
  tap "done"
  expect state doneDays.0 == "2026-03-15"
  expect text "status" == "Done today"
```

- **The write reads the clock itself.** The minute-ticked `now` can be a minute behind, so an
  entry keyed by `today` just after midnight lands on yesterday: the action takes
  `let at = time.epochAtZero + performanceNow()` and keys `formatDate(at, time.utcOffset, "iso")`.
- A derive that reads the clock doesn't update by itself: keep the time in state that a
  `task … every` writes. Store `time.epochAtZero + performanceNow()`, never `performanceNow()`
  alone (0 at each launch).
- `"iso"` is the day key, compared as text. A label is `formatDay(then, shown, offset,
  mode)`, never `day == today ? "Today" : …`: `"day"` is `Today`, `Yesterday`, `Tomorrow` or
  `Mar 14, 2026`; `"day-time"` adds ` at 11:30 PM`; `"list"` is Mail's (the time today,
  `Yesterday`, a weekday this week, else `3/7/26`).
- `utcOffset` is the zone's offset now, so `shown - 86400000` and `formatDay` can miss by an hour
  across a DST change. Tests pin `epoch` and `time-zone`; the default is 2026-01-01 UTC.
