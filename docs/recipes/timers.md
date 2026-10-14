# Recipe: timers

A `task` is the only timer: `every(ms, action)` repeats, `after(ms, action)` fires once,
`when cond` arms it while it holds, so start and stop are a state. A countdown, its
readout and button outside the list the route scrolls, so they stay put (inside it they
scroll off under the bar):

```contract
routes nav
  home "/"

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
  action toggle
    now = performanceNow()
    endsAt = running ? 0 : performanceNow() + minutes * 60000
    finished = false
  action choose(m: number)
    minutes = m
    finished = false
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="presets" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Tea" role="heading" aria-level=1
          column align-items="center" gap=12 padding=16
            text mmss(left) font-size="4.5rem" font-variant-numeric="tabular-nums" testId="left"
            text (finished ? "Ready" : running ? "Steeping" : " ") testId="status"
            button press=toggle buttonStyle="bordered-prominent" testId="start-stop"
              text (running ? "Stop" : "Start")
          list id="presets" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              each m in [1, 3, 5, 10] key=m
                button press=choose(m) disabled=running testId=`min-${m}`
                  text `${m} min`
                  when minutes == m
                    image "symbol:checkmark"
```

```contract-test
test "a countdown ends at zero"
  tap "min-1"
  tap "start-stop"
  clock +30000
  expect text "left" == "0:30"
  clock +31000
  expect text "left" == "0:00"
  expect text "status" == "Ready"
```

- **The clock comes in through the task:** `performanceNow()` in a derive is not read again
  as time passes, so the task's action writes it into state (`now`) and the view follows.
  Tick often, compute from the end (`endsAt - now`): counting ticks drifts.
- `key=x` re-arms a task when `x` changes (a debounce: `task save key=draft`, `after(500,
  save)`); `task poll mount` runs for the app's life ([rest-backend](rest-backend.md)).
  A test's `clock +N` fires the timers it passes.
- `performanceNow()` restarts at 0 each launch: a timer that outlives a quit stores
  `time.epochAtZero + performanceNow()` ([timer-that-survives-relaunch](timer-that-survives-relaunch.md)).
