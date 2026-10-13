# Recipe: a timer that survives a relaunch

`performanceNow()` restarts at 0 on every launch, so a start mark that outlives it is a date,
`time.epochAtZero + performanceNow()`. It is a small number: a root `state … persist` keeps it, with
no data module ([persisted-setting](persisted-setting.md)).

```contract
shape Time
  epochAtZero: number

fn pad2(n: number): string = n < 10 ? `0${n}` : `${n}`
fn clock(ms: number): string = `${floor(ms / 60000)}:${pad2(floor(ms / 1000) % 60)}`

component Stopwatch
  state startedAt = 0 persist
  state banked = 0 persist
  state now = 0
  resource time = exactTime() as shape Time
  derive running = startedAt > 0
  derive elapsed = banked + (running ? max(0, time.epochAtZero + now - startedAt) : 0)
  task tick when running
    every(250, step)
  action step
    now = performanceNow()
  action toggle
    let at = time.epochAtZero + performanceNow()
    now = performanceNow()
    banked = running ? banked + at - startedAt : banked
    startedAt = running ? 0 : at
  action reset
    banked = 0
  view
    column align-items="center" gap=16 padding=32
      text clock(elapsed) font-size=72 font-variant-numeric="tabular-nums" testId="elapsed"
      row gap=12
        button press=toggle buttonStyle="bordered-prominent" testId="start-stop"
          text (running ? "Stop" : "Start")
        button press=reset disabled=(running or elapsed == 0) buttonStyle="bordered" testId="reset"
          text "Reset"
```

```contract-test
test "a running timer keeps its start mark across a relaunch"
  epoch "2026-10-10T09:30:00Z"
  tap "start-stop"
  clock +5000
  expect text "elapsed" == "0:05"
  reload
  expect text "elapsed" == "0:05"
  expect state startedAt == 1791624600000

test "stop banks the time, and reset clears it"
  tap "start-stop"
  clock +65000
  tap "start-stop"
  reload
  expect text "elapsed" == "1:05"
  tap "reset"
  expect text "elapsed" == "0:00"
```

- Store dates (`time.epochAtZero + performanceNow()`), never `performanceNow()`: after a relaunch
  a stored one is a moment in 1970 and the elapsed time goes negative. Time spent before a pause
  is a duration (`banked`), safe to keep.
- Read `performanceNow()` in the action itself, not the `now` the timer last wrote, which can be
  250 ms old.
- A test's `reload` relaunches at the virtual time the test had reached, so a running timer reads
  as it did before it (`0:05` above); `performanceNow()` starts at 0 again.
- The readout is deliberate design: its size, with `tabular-nums` ([numeric-readout](numeric-readout.md)).
