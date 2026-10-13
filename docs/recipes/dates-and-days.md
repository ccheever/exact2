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
    doneDays = concat(doneDays, [formatDate(at, time.utcOffset, "iso")])
  view
    column padding=16 gap=8
      text formatDate(shown, time.utcOffset, "medium") role="heading" aria-level=2 testId="today"
      text `Updated ${formatTime(shown, time.utcOffset, "short")}` color="-exact-secondary-label" testId="updated"
      if includes(doneDays, today)
        text "Done today" testId="status"
      else if includes(doneDays, yesterday)
        text "Done yesterday" testId="status"
      else
        text "Not done yet" testId="status"
      button press=markDone disabled=includes(doneDays, today) buttonStyle="filled" testId="done"
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

test "an entry made 30 s after midnight, before the tick, is the new day's"
  epoch "2026-03-15T06:59:50Z"
  time-zone "America/Los_Angeles"
  clock +40000
  expect text "today" == "Mar 14, 2026"
  tap "done"
  expect state doneDays.0 == "2026-03-15"
  expect text "status" == "Done today"
```

- **The write reads the clock itself.** The minute-ticked `now` is for display and can be a
  minute behind, so an entry keyed by `today` just after midnight lands on yesterday: the action
  takes `let at = time.epochAtZero + performanceNow()` and keys `formatDate(at, time.utcOffset,
  "iso")` (an action reads the state it started with, so `today` is still the old day in it).
- A derive that reads the clock doesn't update by itself: keep the time in state that a
  `task … every` writes (a minute for a date, a second or less for a timer). Store
  `time.epochAtZero + performanceNow()`, never `performanceNow()` alone, which restarts at 0 each
  launch ([timer-that-survives-relaunch](timer-that-survives-relaunch.md)).
- `"iso"` is the day key and keys compare as strings; `"medium"` is `Mar 14, 2026`,
  `formatTime(…, "short")` is `11:30 PM`.
- `utcOffset` is the zone's offset now (a DST change arrives with the next tick), so
  `shown - 86400000` can miss by an hour across one. Tests pin `epoch` and `time-zone`; the
  driver's default is 2026-01-01T00:00:00Z in UTC.
