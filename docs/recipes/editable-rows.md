# Recipe: rows added and removed while editing

A poll's options, a recipe's steps: each row's remove is a red `minus-circle`
(`destructive=true`), shown while a row may go, and its swipe (`delete=`); Add is the section's
last row. Removing a row asks nothing; the form's Save or Cancel is the commitment.

```contract
component Options
  state options = ["", ""]
  action edit(i: number, v: string)
    options = options.slice(0, i).concat([v]).concat(options.slice(i + 1))
  action add
    options = options.concat([""])
  action remove(i: number)
    options = options.filter((o, j) => j != i)
  view
    list appearance="auto" listStyle="inset-grouped" height="100%"
      section
        header
          text "Options"
        for o, i in options key=i
          row gap=8 align-items="center" delete=remove(i) disabled=(options.length <= 2)
            if options.length > 2
              button press=remove(i) destructive=true aria-label=`Remove option ${i + 1}` testId=`remove-${i}`
                image "symbol:minus-circle"
            input value=o input=edit(i) placeholder=`Option ${i + 1}` aria-label=`Option ${i + 1}`
              testId=`option-${i}` flex=1 min-width=0
        if options.length < 6
          button press=add testId="add-option"
            text "Add Option"
```

```contract-test
test "an option is added and removed, never below two"
  expect tree missing "remove-0"
  tap "add-option"
  type "option-2" "Tacos"
  tap "remove-0"
  expect state options.1 == "Tacos"
  expect tree missing "option-2"
  expect tree missing "remove-0"
```

- What can't be undone once saved (Delete, Close Poll, Cancel Event) asks first (`contract vocab
  confirm`).
