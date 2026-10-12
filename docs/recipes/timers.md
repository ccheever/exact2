# Recipe: timers

A `task` is the only timer: `every(ms, action)` repeats, `after(ms, action)` fires once.
`when cond` arms it while the condition holds, so starting and stopping is a state. A
countdown:

```contract
fn pad2(n: number): string = n < 10 ? `0${n}` : `${n}`
fn mmss(ms: number): string = `${floor(ceil(ms / 1000) / 60)}:${pad2(ceil(ms / 1000) % 60)}`

component Tea
  state minutes = 3
  state endsAt = 0
  state now = 0
  state finished = false
  derive running = endsAt > 0
  derive left = running ? max(0, endsAt - now) : finished ? 0 : minutes * 60000
  task tick when running
    every(250, step)
  action step
    now = performanceNow()
    if performanceNow() >= endsAt
      endsAt = 0
      finished = true
  action start
    now = performanceNow()
    endsAt = performanceNow() + minutes * 60000
    finished = false
  action stop
    endsAt = 0
  action choose(m: number)
    minutes = m
    finished = false
  view
    column align-items="center" gap=20 padding=32 testId="app"
      row role="radiogroup" aria-label="Minutes" width="100%"
        each m in [1, 3, 5] key=m
          button role="radio" aria-checked=(minutes == m) press=choose(m) disabled=running testId=`min-${m}` flex=1
            text `${m} min`
      text mmss(left) font-size="4.5rem" font-variant-numeric="tabular-nums" testId="left"
        aria-label=`${mmss(left)} left`
      text (finished ? "Ready" : running ? "Steeping" : " ") testId="status"
        color=(finished ? "-exact-system-green" : "-exact-secondary-label")
      when running
        button press=stop buttonStyle="bordered" testId="start-stop"
          text "Stop"
      else
        button press=start buttonStyle="bordered-prominent" testId="start-stop"
          text "Start"
```

```contract-test
test "a countdown ticks down, stops, and ends at zero"
  tap "min-1"
  tap "start-stop"
  clock +30000
  expect text "left" == "0:30"
  tap "start-stop"
  expect state running == false
  tap "start-stop"
  clock +61000
  expect text "left" == "0:00"
  expect text "status" == "Ready"
```

- **The clock comes in through the task.** `performanceNow()` read in a derive is not
  read again as time passes, so the task's action writes it into state (`now`) and the
  view follows that.
- **Start and stop** set the gate: `task tick when running` has its timer only while
  `running` holds, armed from the commit that made it true. `key=x` re-arms it whenever
  `x` changes (a debounce: `task save key=draft` with `after(500, save)`).
- **`task poll mount`** runs from launch for as long as the app does (polling a server:
  [rest-backend](rest-backend.md)); `every(frame, step)` ticks once a frame, for motion.
- The interval is a literal number of milliseconds, and the action takes no parameters.
- **Tick often, compute from the end.** `every(250, …)` with `endsAt - now` stays right
  however late a tick fires; counting ticks drifts.
- In tests `clock +N` moves the virtual clock and fires the timers on the way; never
  wait in real time.
- `performanceNow()` restarts at 0 on every launch: a timer that must outlive a quit
  stores `time.epochAtZero + performanceNow()` ([timer-that-survives-relaunch](timer-that-survives-relaunch.md)).
