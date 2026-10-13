# Recipe: a grouped form

A settings form: a grouped list with a text field, a quantity, a switch and a
date picker, validated after the first Save.

```contract
routes nav
  home "/"

component Profile
  state name = ""
  state qty = "1"
  state reminders = true
  state birthday = ""
  state submitted = false
  derive nameError = trim(name) == "" ? "Enter your name." : ""
  action editName(v: string)
    name = v
  action editQty(v: string)
    qty = v
  action setReminders(on: bool)
    reminders = on
  action setBirthday(v: string)
    birthday = v
  action save
    submitted = true
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="form" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Profile" role="heading" aria-level=1 flex=1
            button press=save buttonStyle="filled" testId="save"
              text "Save"
          list id="form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                column flex=1 min-width=0 gap=4
                  input value=name input=editName submit=save placeholder="Name" aria-label="Name"
                    autocomplete="name" aria-invalid=(submitted and nameError != "") testId="name"
                  when submitted and nameError != ""
                    text nameError font="-exact-footnote" color="-exact-system-red" testId="name-error"
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
test "an error shows after the first Save and clears when fixed"
  expect tree missing "name-error"
  type "name" key "Enter"
  expect text "name-error" == "Enter your name."
  type "name" "Ada"
  tap "save"
  expect tree missing "name-error"
  expect state submitted == true
```

- Leave a field in a grouped row unsaid: each platform draws its own, as it does a switch, a
  date picker and a menu (`select` of `option "Metric" value="metric"`).
- A quantity from 1 to 99 is `input type="number" min=1 max=99 step=1`: the platform draws its
  stepper; the field stays editable and its `value` is text (`parseNumber` it).
- A `text` and a control in a row are a title and its accessory; any other is a flex row.
  Show a validation derive only after the first Save, under the field in `-exact-system-red`
  with `aria-invalid`; `submit=save` makes Enter save (no `form`).
