# Recipe: an edit sheet a row opens

A row that opens its item's sheet is a `link`: a tap, a deep link and Back arrive at
`follow`, which runs nothing of the row's, so an action that fills a draft (`openEdit`)
never runs and the sheet shows the last draft. Make the form a child whose states start
from the item when its route shows it; the item's `key` makes it again for another item.

```contract
routes nav
  lifts "/"
    edit "/edit/:id"

shape Lift
  id: string
  name: string
  reps: number

component App
  state lifts = [Lift(id="1", name="Squat", reps=10), Lift(id="2", name="Row", reps=8)]
  action follow(url: string)
    nav = go(nav, url)
  action back
    nav = back(nav)
  action save(id: string, reps: number)
    lifts = lifts.map(l => l.id == id ? Lift(l, reps=reps) : l)
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
      for e in stack(nav) key=e.id
        if e.name == "lifts"
          column navigationKey=`${e.id}` position="absolute" inset=0
            list appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
              section
                for l in lifts key=l.id
                  link href=path("edit", l.id) testId=`lift-${l.id}`
                    text `${l.name} × ${l.reps}`
        if e.name == "edit"
          column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
            for l in lifts.filter(x => x.id == e.params.id) key=l.id
              EditLift(lift=l, save=save, cancel=back)

component EditLift
  props
    lift: Lift
    save: action
    cancel: action
  state reps = `${lift.reps}`
  derive n = match parseNumber(reps) { case some(v) => v, case none => 0 }
  action edit(v: string)
    reps = v
  view
    column flex=1 min-height=0
      header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
        button id="back" press=cancel aria-label="Cancel"
          image "symbol:close"
        text lift.name role="heading"
        button press=save(lift.id, n) disabled=(n < 1) buttonStyle="bordered-prominent" aria-label="Save" testId="save"
          image "symbol:checkmark"
      list appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
        section
          row
            text "Reps"
            input value=reps input=edit inputmode="numeric" aria-label="Reps" text-align="end" flex=1 min-width=0 testId="reps"
```

```contract-test
test "each row's sheet opens with that row's values"
  tap "lift-1"
  clock settle
  expect text "reps" == "10"
  type "reps" "12"
  tap "save"
  clock settle
  expect state lifts.0 .reps == 12
  tap "lift-2"
  clock settle
  expect text "reps" == "8"
```

- Items `app.ts` keeps work the same way: the child is made once its item has loaded.
- Test a second row: a sheet showing the last draft passes a test whose row matches it.
