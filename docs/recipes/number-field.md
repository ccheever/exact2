# Recipe: a number field

`inputmode="decimal"` gives the iPhone's number pad with a decimal point, and
`inputmode="numeric"` digits only. Without one, a number field gets the full keyboard,
with QuickType inserting words. The field holds raw text; a derive parses it.

```contract
routes nav
  home "/"

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
    people = n >= 1 ? n : people
    peopleText = n >= 1 ? `${n}` : `${people}`

  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="form" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Split" role="heading" aria-level=1
          list id="form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                text "Bill"
                input appearance="none" value=billText input=editBill inputmode="decimal" placeholder="0.00"
                  aria-label="Bill" text-align="end" testId="bill" flex=1 min-width=0
              row
                text "People"
                input appearance="none" value=peopleText input=editPeople change=commitPeople inputmode="numeric"
                  aria-label="People" text-align="end" testId="people" flex=1 min-width=0
              footer
                text billError testId="bill-error"
            section
              row
                text "Each Pays"
                text (bill > 0 ? toFixed(bill / people, 2) : "—") font-variant-numeric="tabular-nums" testId="share"
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

- `input` writes the raw text to state on every keystroke, and validation reads a
  derive of it. Normalize only in `change` (Enter or blur), never as the person types:
  rewriting the text mid-edit puts the next keystroke in the wrong place.
- `parseNumber` answers an option: `none` for `""`, `"12px"` or `"1.2.3"`. Show a
  hint in words; leave the text as typed.
- The number pad has no Return key, so `change` comes on blur. A `scroll` with
  `keyboardDismissMode="on-drag"` lets a drag put the keyboard away.
- Show money with [money](money.md), and a big result with [numeric-readout](numeric-readout.md).
