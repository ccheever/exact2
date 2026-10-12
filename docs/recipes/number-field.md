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
    let next = n >= 1 ? n : people
    people = next
    peopleText = `${next}`

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
                column flex=1 min-width=0 gap=4
                  row gap=8
                    text "Bill"
                    input value=billText input=editBill inputmode="decimal" placeholder="0.00"
                      aria-label="Bill" text-align="end" testId="bill" flex=1 min-width=0
                  when billError != ""
                    text billError font="-exact-footnote" color="-exact-system-red" testId="bill-error"
              row
                text "People"
                input value=peopleText input=editPeople change=commitPeople inputmode="numeric"
                  aria-label="People" text-align="end" testId="people" flex=1 min-width=0
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
- The number pad has no Return key, so `change` comes on blur. In a `scroll` or `list`
  a drag that reaches the keyboard puts it away; `keyboardDismissMode="on-drag"` does
  as the drag starts.

## A value that can be negative

The decimal pad has no minus key. Say the field's range: a `min` below zero gives it a ±
key on iOS, in the keyboard's bar, as a hand-built converter adds one (the web's keyboard
has a minus). Never `inputmode="text"` for a number: that is a letter keyboard.

```contract
component Convert
  state celsiusText = ""
  derive celsius = parseNumber(celsiusText)
  action edit(v: string)
    celsiusText = v
  view
    column gap=8 padding=16
      input value=celsiusText input=edit inputmode="decimal" min="-273.15" enterkeyhint="done"
        placeholder="0" aria-label="Celsius" testId="celsius"
      text (match celsius { case some(c) => `${toFixed(c * 9 / 5 + 32, 1)} °F`, case none => "Enter °C" })
        font-variant-numeric="tabular-nums" testId="fahrenheit"
```

```contract-test
test "below zero converts"
  type "celsius" "-40"
  expect text "fahrenheit" == "-40.0 °F"
```

- **Putting the keyboard away** is the field's, not a permanent Done button in the bar:
  `enterkeyhint="done"` gives the pad a Done key that puts it away (and fires `change`),
  `blur()` does from the action that uses the value, and a drag on the scroller that
  reaches the keyboard does too.
- Show money with [money](money.md), and a big result with [numeric-readout](numeric-readout.md).

## A number in a form that saves

```contract
fn minutes(s: string): number = match parseNumber(s) { case some(n) => n, case none => -1 }
fn valid(n: number): bool = n >= 1 and n <= 60 and floor(n) == n

component Settings
  state focus = 25
  state draft = "25"
  derive bad = not valid(minutes(draft))
  action edit(v: string)
    draft = v
  action save
    if not bad
      focus = minutes(draft)
  view
    column gap=8 padding=16
      input value=draft input=edit submit=save inputmode="numeric" aria-label="Focus minutes"
        aria-invalid=bad aria-describedby="focus-error" testId="focus"
      when bad
        text "Enter a whole number from 1 to 60." id="focus-error" font="-exact-footnote"
          color="-exact-system-red" testId="focus-error"
      button press=save disabled=bad buttonStyle="bordered-prominent" testId="save"
        text "Save"
      text `Focus: ${focus} min` testId="saved"
```

```contract-test
test "Enter saves a whole number; a bad one is marked and not saved"
  type "focus" "0"
  expect tree has "focus-error"
  type "focus" key "Enter"
  expect text "saved" == "Focus: 25 min"
  type "focus" "40"
  type "focus" key "Enter"
  expect text "saved" == "Focus: 40 min"
```

- `submit=save` on the field: Enter saves, as on a web form, and so does Return on iOS
  (the number pad gets a Done key for it). There is no `form`, so a field without
  `submit` saves nothing on Enter.
- `aria-invalid` while the text is bad, and `aria-describedby` naming the error: the web
  outlines the field in the system red, and a screen reader reads the error with it.
- `disabled` on Save while the text is bad: it draws disabled, grey whatever the accent,
  as a platform button does. Leave its colours unsaid.
