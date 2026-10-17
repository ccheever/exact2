# Recipe: a grouped form

An entry or settings form: a grouped list with a text field, a quantity, a switch and
a date picker. Save is disabled until the form is valid, saying why.

```contract
routes nav
  home "/"

component Profile
  state name = ""
  state qty = "1"
  state reminders = true
  state birthday = ""
  state saved = false
  derive nameHint = name.trim() == "" ? "Enter your name to save." : ""
  action editName(v: string)
    name = v
  action editQty(v: string)
    qty = v
  action setReminders(on: bool)
    reminders = on
  action setBirthday(v: string)
    birthday = v
  action save
    if nameHint == ""
      saved = true
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="form" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Profile" role="heading" aria-level=1 flex=1
            button press=save buttonStyle="filled" disabled=(nameHint != "") testId="save"
              text "Save"
          list id="form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                input value=name input=editName submit=save placeholder="Name" aria-label="Name"
                  autocomplete="name" autocapitalize="words" testId="name" flex=1 min-width=0
              footer
                text nameHint testId="name-hint"
            section
              row
                text "Quantity"
                input type="number" value=qty input=editQty min=1 max=99 step=1 aria-label="Quantity" testId="qty"
              row
                text "Reminders"
                input type="checkbox" switch checked=reminders change=setReminders aria-label="Reminders" testId="reminders"
              row
                text "Birthday"
                input type="date" value=birthday change=setBirthday aria-label="Birthday" testId="birthday"
```

```contract-test
test "Save waits for a name, saying why; Enter saves"
  expect disabled "save"
  expect text "name-hint" == "Enter your name to save."
  type "name" "Ada"
  expect enabled "save"
  expect text "name-hint" == ""
  type "name" key "Enter"
  expect state saved == true
```

- Leave a field in a grouped row unsaid: each platform draws its own, as it does a switch, a
  date picker and a menu (`select` of `option "Metric" value="metric"`).
- A quantity from 1 to 99 is `input type="number" min=1 max=99 step=1`: the platform draws its
  stepper; the field stays editable and its `value` is text (`parseNumber` it).
- A `text` and a control in a row are a title and its accessory; any other is a flex row.
  Save is disabled until valid, the reason in the section's `footer`; red under a field is a
  refusal the data sent back ([after-a-write](after-a-write.md)), kept until a Save succeeds.
  `submit=save` makes Enter save (no `form`).
