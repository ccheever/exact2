# Recipe: a number field

`inputmode="decimal"` is the iPhone's number pad with a decimal point, `"numeric"` digits only;
without one, the full keyboard. `type="number"` gets the number pad when its `step` and `min`
are whole or unsaid (else the decimal pad), and the platform's stepper, which adds `step`: for a
count, or a value in a unit (`step=50` ml). A typed amount is `inputmode` alone, as `Bill` is.
The field holds raw text; a derive parses it.

```contract
fn amount(s: string): number = match parseNumber(s) { case some(n) => n, case none => -1 }

component Split
  state billText = ""
  state people = 2
  state peopleText = "2"
  derive bill = amount(billText)
  derive billError = billText != "" && bill < 0 ? "Enter an amount, like 42.50." : ""
  action editBill(v: string)
    billText = v
  action editPeople(v: string)
    peopleText = v
  action commitPeople(v: string)
    let n = round(amount(v))
    let next = n >= 1 ? n : people
    people = next
    peopleText = `${next}`
  view
    list appearance="auto" listStyle="inset-grouped" height="100%" testId="app"
      section
        row
          text "Bill"
          input value=billText input=editBill inputmode="decimal" placeholder="0.00" aria-label="Bill"
            aria-invalid=(billError != "") text-align="end" testId="bill" flex=1 min-width=0
        row
          text "People"
          input value=peopleText input=editPeople change=commitPeople inputmode="numeric"
            aria-label="People" text-align="end" testId="people" flex=1 min-width=0
        footer
          text billError testId="bill-error"
      section
        row
          text "Each Pays"
          text (bill > 0 ? (bill / people).toFixed(2) : "—") font-variant-numeric="tabular-nums" testId="share"
```

```contract-test
test "a bad amount says so, and people commit on Enter"
  type "bill" "12.5.0"
  expect text "bill-error" == "Enter an amount, like 42.50."
  type "bill" "84"
  expect text "share" == "42.00"
  type "people" "0"
  type "people" key "Enter"
  expect text "people" == "2"
  type "people" "3"
  type "people" key "Enter"
  expect text "share" == "28.00"
```

- `input` writes the raw text at each keystroke; normalize only in `change` (Enter or blur),
  never as the person types: a rewrite mid-edit misplaces the next keystroke. `parseNumber` answers `none` for `""`, `"12px"`: say so in words (the footer, not red:
  red is a refusal the data sent back) and leave the text as typed.
- The number pad has no Return key: iOS gives it a Done bar that puts it away and fires
  `change`, as does a drag that reaches the keyboard.
- A value that can be negative: the decimal pad has no minus key, so say the range: `min=-273.15`
  (bare) gives iOS a ± key in the keyboard's bar. Never `inputmode="text"` for a number.
- In a form that saves: `submit=save` on the field (Enter saves; no `form`), `aria-invalid`
  while the text is bad, Save disabled until valid, saying why ([grouped-form](grouped-form.md)). Money: [money](money.md); a big result: [numeric-readout](numeric-readout.md).
