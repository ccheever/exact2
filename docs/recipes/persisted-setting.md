# A remembered setting

A setting the app keeps across launches — a tip percentage, °C or °F — is a root
`state` that ends in `persist` (LLP 1116 D5). There is no data module, grant or
JSON to write: the runner reads the store before the first frame and writes it
after every commit that changes the state.

```contract
component App
  state tipPercent = 18 persist
  state units = "C" persist
  action tip(p: number)
    tipPercent = p
  action unit(u: string)
    units = u
  view
    column padding=16 gap=12
      text "Tip" role="heading" aria-level=2
      row role="tablist" aria-label="Tip"
        each p in [15, 18, 20, 25] key=p
          button role="tab" aria-selected=(tipPercent == p) press=tip(p) testId=`tip-${p}` flex=1
            text `${p}%`
      text "Temperature" role="heading" aria-level=2
      row role="tablist" aria-label="Temperature"
        button role="tab" aria-selected=(units == "C") press=unit("C") testId="unit-c" flex=1
          text "°C"
        button role="tab" aria-selected=(units == "F") press=unit("F") testId="unit-f" flex=1
          text "°F"
```

The test, in `app.test.contract`. Each test starts from an empty store, as a
fresh install does; `reload` relaunches on the store the test has so far:

```contract-test
test "the settings survive a relaunch"
  tap "tip-20"
  tap "unit-f"
  reload
  expect state tipPercent == 20
  expect state units == "F"

test "a fresh install starts from the declared values"
  expect state tipPercent == 18
  expect state units == "C"
```

Run it with `bun exact.mjs test web` (or `test ios`, `test linux`). To see it by
hand, drive twice with the same store: `bun exact.mjs agent web --storage s1 "tap
tip-20"`, then `bun exact.mjs agent web --storage s1 state` shows `tipPercent:
20`. A drive without `--storage` starts from the declared values and keeps nothing
past the drive.

What to know:

- **The first frame.** A kept value of the state's type replaces the declared one.
  A missing value keeps the declared one, and so does a value of another shape
  (the state's type changed since it was kept); `logs` names that one.
- **Where.** The page's `localStorage` (`exact.state.<name>`), the app's
  `UserDefaults` domain on Apple (never the Keychain), a file in the app's data
  directory on Linux. The key is the state's name: renaming the state forgets it.
- **Only small settings.** A number, string or bool, or an option or list of one,
  at most 16 KB as JSON. `persist` on a record or a list of records is refused
  (`type-persist-type`): keep app data in a source. A child component's state
  cannot persist (`type-persist-child`): keep the setting in the root and pass it
  down as a prop.
- **A derive is never persisted.** Persist what it reads (`syntax-persist-derive`).
