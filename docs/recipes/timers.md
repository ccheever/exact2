# Recipe: timers

A `task`, the root's ([a child's](timer-in-a-component.md)), is the only timer: `every(ms,
action)` repeats, `after(ms, action)` fires once, `when cond` arms it while it holds, so
start and stop are a state. A countdown, its readout and button outside the list the
route scrolls, so they stay put:

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
      for e in stack(nav) key=e.id
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
              for m in [1, 3, 5, 10] key=m
                button press=choose(m) disabled=running testId=`min-${m}`
                  text `${m} min`
                  if minutes == m
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

- A task writes `performanceNow()` into state; a derive does not update as time passes. Compute from the end; counting ticks drifts.
- `key=x` restarts a task when `x` changes. A test's `clock +N` fires the timers it passes.
- `performanceNow()` restarts at launch. Store `time.epochAtZero + performanceNow()` for one that survives a quit ([recipe](timer-that-survives-relaunch.md)).

For deferrable work, use `every(5000, tick, wait=scrollend, max=1000)` or `after(800, save, wait=idle)`. `wait=frame` joins the next frame; `wait=update` joins existing frame demand. For wake coalescing, use `tolerance=500` instead of `wait=`. See [options](../reference.md#timer-options).
