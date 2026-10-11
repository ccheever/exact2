# Recipe: a timer that survives a relaunch

`performanceNow()` is milliseconds since this launch of the app: it restarts at 0 on
every launch. A start mark that outlives the launch is a date,
`time.epochAtZero + performanceNow()`, saved by the data module.

```contract
shape Time
  epochAtZero: number
  utcOffset: number

shape Timer
  startedAt: number
  banked: number

shape Ack
  ok: bool

fn pad2(n: number): string = n < 10 ? `0${n}` : `${n}`
fn clock(ms: number): string = `${floor(ms / 60000)}:${pad2(floor(ms / 1000) % 60)}`

component Stopwatch
  state now = 0
  resource time = exactTime() as shape Time
  resource timer = loadTimer() as shape Timer else empty()
  mutation saved as shape Ack queue refreshes timer
  derive running = timer.startedAt > 0
  derive elapsed = timer.banked + (running ? max(0, time.epochAtZero + now - timer.startedAt) : 0)
  task tick when running
    every(250, step)
  action step
    now = performanceNow()
  action start
    now = performanceNow()
    send saved = saveTimer(time.epochAtZero + performanceNow(), timer.banked)
  action stop
    send saved = saveTimer(0, timer.banked + time.epochAtZero + performanceNow() - timer.startedAt)
  action reset
    send saved = saveTimer(0, 0)
  view
    column align-items="center" gap=16 padding=32
      text clock(elapsed) font-size=72 font-variant-numeric="tabular-nums" testId="elapsed"
      text (running ? `Started ${formatTime(timer.startedAt, time.utcOffset, "short")}` : " ")
        color="-exact-secondary-label" testId="started"
      row gap=12
        when running
          button press=stop buttonStyle="bordered-prominent" testId="start-stop"
            text "Stop"
        else
          button press=start buttonStyle="bordered-prominent" testId="start-stop"
            text "Start"
        button press=reset disabled=(running or elapsed == 0) buttonStyle="bordered" testId="reset"
          text "Reset"
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.stopwatch'; // exact new writes this line
export const grants = ['fs.read app:/data/timer.json', 'fs.write app:/data/timer.json'].join('\n');
const FILE = 'app:/data/timer.json';

const sources: Sources = {
  loadTimer: (_args, _store, storage) => storage.fs.readFile(FILE).then(
    (bytes): Result<'loadTimer'> => JSON.parse(new TextDecoder().decode(bytes)),
    (e) => { if (e.code === 'ENOENT') return { startedAt: 0, banked: 0 }; throw e; }),
  saveTimer: async ([startedAt, banked], _store, storage) => {
    await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify({ startedAt, banked })));
    return { ok: true };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a running timer keeps its start mark across a relaunch"
  epoch "2026-10-10T09:30:00Z"
  tap "start-stop"
  clock data
  clock +5000
  expect text "elapsed" == "0:05"
  reload
  expect text "started" == "Started 9:30 AM"
  expect state timer.startedAt == 1791624600000

test "stop banks the time, and reset clears it"
  tap "start-stop"
  clock data
  clock +65000
  tap "start-stop"
  clock data
  reload
  expect text "elapsed" == "1:05"
  tap "reset"
  clock data
  expect text "elapsed" == "0:00"
```

- Store dates (`time.epochAtZero + performanceNow()`), never `performanceNow()`: after a
  relaunch a stored `performanceNow()` is a moment in 1970 and the elapsed time goes
  negative. Time spent before a pause is a duration (`banked`), which is safe to keep.
- Read `performanceNow()` in the action itself (`start`, `stop`), not the `now` the
  timer last wrote, which can be 250 ms old. The display follows `now`, which a `task …
  every` writes while the timer runs.
- Under the driver, `reload` starts the clock over at the test's epoch, so a running
  timer reads less after it than before. Assert the saved mark, as above; a stored
  `performanceNow()` shows "Started 12:00 AM" there.
- The readout is deliberate design: its size, with `tabular-nums` so digits don't
  shift ([numeric-readout](numeric-readout.md)).
