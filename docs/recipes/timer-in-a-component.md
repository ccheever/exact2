# Recipe: a component that needs a timer

Only the root may declare a `task` (`type-child-resource`): a timer is the app's. A
component that starts and stops one, a capsule a held finger charges, keeps its view; the
task, the state that gates it and the actions that set that state move to the root, and the
component takes the state and the actions as props ([timers](timers.md) has the rest):

```contract
component App
  state charge = 0
  state holding = false
  task charging when holding
    every(16, step)
  action step
    charge = min(100, charge + 2)
  action hold(on: bool)
    holding = on
  view
    column padding=16 gap=12
      Capsule(charge=charge, down=hold(true), up=hold(false))

component Capsule
  props
    charge: number
    down: action
    up: action
  view
    column gap=8
      button pointerdown=down pointerup=up testId="capsule"
        text (charge < 100 ? "Hold to charge" : "Charged")
      progress value=charge max=100
```

```contract-test
test "a hold charges the capsule, and letting go stops it"
  tap "capsule" drag 0 0 hold 400
  expect state charge == 50
  clock +500
  expect state charge == 50
  tap "capsule" drag 0 0 hold 1000
  expect state charge == 100
  expect text "capsule" == "Charged"
```

- `hold(true)` is bound where the component is used, so `pointerdown=down` runs the root's
  action with `true`; the component never writes the root's state.
- Two capsules gate two tasks, or one task on either flag (`when leftHeld or rightHeld`)
  whose action steps the one held.
