# Recipe: a grouped form

A settings-style form: an inset-grouped list of sections, fields without a border of
their own, a switch, a pop-up menu and a date picker, validated after the first Save.

```contract
routes nav
  home "/"

shape Ack
  ok: bool

component Profile
  state name = ""
  state email = ""
  state reminders = true
  state units = "metric"
  state birthday = ""
  state submitted = false
  mutation saved as shape Ack
  derive nameError = trim(name) == "" ? "Enter your name." : ""
  derive emailError = includes(email, "@") ? "" : "Enter an email address, like name@example.com."

  action editName(v: string)
    name = v
  action editEmail(v: string)
    email = v
  action setReminders(on: bool)
    reminders = on
  action setUnits(v: string)
    units = v
  action setBirthday(v: string)
    birthday = v
  action save
    submitted = true
    if nameError == "" and emailError == ""
      send saved = saveProfile(trim(name), trim(email), reminders, units, birthday)

  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="form" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
            text "Profile" role="heading" aria-level=1
            button press=save disabled=pending(saved) testId="save"
              text "Save"
          list id="form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              header
                text "Name"
              row
                input appearance="none" value=name input=editName placeholder="Required" aria-label="Name"
                  autocomplete="name" testId="name" flex=1
              footer
                text (submitted ? nameError : "") testId="name-error"
            section
              header
                text "Email"
              row
                input appearance="none" type="email" value=email input=editEmail placeholder="name@example.com"
                  aria-label="Email" autocapitalize="off" autocomplete="email" testId="email" flex=1
              footer
                text (submitted ? emailError : "") testId="email-error"
            section
              header
                text "Preferences"
              row
                text "Reminders"
                input type="checkbox" switch checked=reminders change=setReminders aria-label="Reminders" testId="reminders"
              row
                text "Units"
                select value=units change=setUnits aria-label="Units" testId="units"
                  option "Metric" value="metric"
                  option "Imperial" value="imperial"
              row
                text "Birthday"
                input type="date" value=birthday change=setBirthday aria-label="Birthday" testId="birthday"
```

```contract-test
test "errors show after the first Save and clear when fixed"
  expect text "name-error" == ""
  tap "save"
  expect text "name-error" == "Enter your name."
  type "name" "Ada"
  type "email" "ada@example.com"
  type "units" "imperial"
  type "reminders" "false"
  tap "save"
  expect text "name-error" == ""
  expect state reminders == false
```

- A text field in a grouped row takes `appearance="none"`, or it draws its own rounded
  box inside the card. Leave the switch, menu and date picker as they are: each is the
  platform's control (`UISwitch`, a pop-up button, `UIDatePicker`).
- A row of a `text` and a switch is a title and its accessory, as Settings draws it. Any
  other row is yours, a flex row: vertical content goes in a `column flex=1`, and it
  writes only `padding-top`/`padding-bottom` (`padding` or `width="100%"` runs it off
  the card).
- Validate a derive of the raw text, and show it only after the first Save. A
  footer's text is always the system's grey, so don't colour it.
- The list paints its own grouped background; don't set one. To keep a setting across
  launches, see [persisted-setting](persisted-setting.md).
