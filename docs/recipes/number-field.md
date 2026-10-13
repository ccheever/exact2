# Recipe: a number field

`inputmode="decimal"` is the iPhone's number pad with a decimal point, `"numeric"` digits only;
without one the field gets the full keyboard. The field holds raw text; a derive parses it.

```contract
fn amount(s: string): number = match parseNumber(s) { case some(n) => n, case none => -1 }

component Split
  state billText = ""
  state people = 2
  state peopleText = "2"
  derive bill = amount(billText)
  derive billError = billText != "" and bill < 0 ? "Enter an amount, like 42.50." : ""
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
          column flex=1 min-width=0 gap=4
            row gap=8
              text "Bill"
              input value=billText input=editBill inputmode="decimal" placeholder="0.00"
                aria-label="Bill" text-align="end" testId="bill" flex=1 min-width=0
            if billError != ""
              text billError font="-exact-footnote" color="-exact-system-red" testId="bill-error"
        row
          text "People"
          input value=peopleText input=editPeople change=commitPeople inputmode="numeric"
            aria-label="People" text-align="end" testId="people" flex=1 min-width=0
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

- `input` writes the raw text on every keystroke; normalize only in `change` (Enter or blur),
  never as the person types: rewriting the text mid-edit puts the next keystroke in the wrong
  place. `parseNumber` answers `none` for `""`, `"12px"`, `"1.2.3"`: show a hint in words and
  leave the text as typed.
- The number pad has no Return key, so iOS gives it a Done bar that puts it away (and fires
  `change`); a drag on the scroller that reaches the keyboard does too.
- A value that can be negative: the decimal pad has no minus key, so say the range: `min=-273.15`
  (bare, a number as `min=0` is) gives iOS a ± key in the keyboard's bar. Never `inputmode="text"` for a number.
- A number in a form that saves: `submit=save` on the field (Enter saves; there is no `form`),
  `aria-invalid` while the text is bad and `aria-describedby` naming the error, `disabled` on Save
  ([grouped-form](grouped-form.md)). Money: [money](money.md); a big result: [numeric-readout](numeric-readout.md).
