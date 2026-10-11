# Recipe: a numeric readout

A timer's or a total's big number is deliberate design: it may set its size. Give it
`font-variant-numeric="tabular-nums"`, so every digit is the same width and the number
doesn't jitter as it changes.

```contract
component Water
  state ml = 750
  state goal = 2000
  action drink(amount: number)
    ml = ml + amount
  view
    column align-items="center" gap=4 padding=24
      text `${ml}` font-size="4.5rem" font-variant-numeric="tabular-nums" aria-label=`${ml} of ${goal} millilitres`
        testId="total"
      text `of ${goal} ml today` color="-exact-secondary-label"
      row gap=12 margin-top=16
        button press=drink(250) buttonStyle="bordered-prominent" testId="add-250"
          text "+250 ml"
        button press=drink(500) buttonStyle="bordered" testId="add-500"
          text "+500 ml"
```

```contract-test
test "the total grows"
  expect text "total" == "750"
  tap "add-250"
  expect text "total" == "1000"
```

- A size in `rem` (`"4.5rem"`) follows the reader's text size (Dynamic Type on iOS); a
  bare number is fixed points. `bun <exact2>/scripts/no-tells.mjs` passes a size on a
  node with `tabular-nums`, and flags one anywhere else.
- Only the number is designed. Its label stays the platform's (`color="-exact-
  secondary-label"`), and a title is a heading, never a big `font-size`.
- Columns of numbers (laps, a ledger) take `tabular-nums` at body size too, so they
  line up.
- Give the readout an `aria-label` that says what the number is; VoiceOver reads the
  label, not the digits alone.
