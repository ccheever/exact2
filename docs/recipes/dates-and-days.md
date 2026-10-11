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
  derive at = time.epochAtZero + now
  derive today = formatDate(at, time.utcOffset, "iso")
  derive yesterday = formatDate(at - 86400000, time.utcOffset, "iso")
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
  action markDone
    doneDays = concat(doneDays, [today])
  view
    column padding=16 gap=8
      text formatDate(at, time.utcOffset, "medium") role="heading" aria-level=2 testId="today"
      text `Updated ${formatTime(at, time.utcOffset, "short")}` color="-exact-secondary-label" testId="updated"
      when includes(doneDays, today)
        text "Done today" testId="status"
      else when includes(doneDays, yesterday)
        text "Done yesterday" testId="status"
      else
        text "Not done yet" testId="status"
      button press=markDone disabled=includes(doneDays, today) testId="done"
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
```

- `performanceNow()` is milliseconds since this launch of the app; it restarts at 0 on
  every launch, so store `time.epochAtZero + performanceNow()`, never
  `performanceNow()` alone ([timer-that-survives-relaunch](timer-that-survives-relaunch.md)).
- A derive that reads the clock doesn't update by itself: keep the time in state that
  a `task … every` writes (a minute for a date, a second or less for a timer).
- `formatDate(ms, time.utcOffset, "iso")` is the day key; keys compare as strings
  (`"2026-03-14" < "2026-03-15"`). `"medium"` is `Mar 14, 2026`, `formatTime(…,
  "short")` is `11:30 PM`; `calendarDiff(from, to, "years")` counts an age.
- `utcOffset` is the zone's offset now, so `at - 86400000` can miss by an hour across a
  DST change; format a past instant in the data module with `new
  Intl.DateTimeFormat(locale, { timeZone })`. Tests pin `epoch` and `time-zone`; the
  driver's default is 2026-01-01T00:00:00Z in UTC.
