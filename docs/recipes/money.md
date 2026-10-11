# Recipe: money

Keep money as a whole number of cents, and format it at the edge with
`formatNumber(dollars, "currency", "USD")`: `$1,481.47`, `-$5.00`, grouped and rounded to
the cent as `Intl.NumberFormat("en-US", {style: "currency", currency: "USD"})` prints it.

```contract
fn money(cents: number): string = formatNumber(cents / 100, "currency", "USD")

component Tip
  state billCents = 148147
  state tipPercent = 18
  derive tipCents = round(billCents * tipPercent / 100)
  action setTip(p: number)
    tipPercent = p
  view
    column padding=16 gap=8
      row justify-content="space-between"
        text "Tip"
        text money(tipCents) font-variant-numeric="tabular-nums" testId="tip"
      row justify-content="space-between"
        text "Total"
        text money(billCents + tipCents) font-variant-numeric="tabular-nums" testId="total"
      row justify-content="space-between"
        text "Tip rate"
        text formatNumber(tipPercent / 100, "percent") testId="rate"
      row role="tablist" aria-label="Tip"
        each p in [15, 18, 20] key=p
          button role="tab" aria-selected=(tipPercent == p) press=setTip(p) testId=`tip-${p}` flex=1
            text `${p}%`
```

```contract-test
test "money is grouped and rounded to the cent"
  expect text "tip" == "$266.66"
  expect text "total" == "$1,748.13"
  expect text "rate" == "18%"
  tap "tip-20"
  expect text "total" == "$1,777.76"
```

- Store and send cents (`round(dollars * 100)` once, where the text is parsed): adding
  binary fractions drifts (`0.1 + 0.2`). Round each computed amount once, to a cent.
- `formatNumber(n, "currency", "<code>")` takes an ISO 4217 code literal (USD, EUR, GBP,
  JPY, …; an unknown code is refused at compile time). `formatNumber(n, "decimal")` is a
  grouped number (`1,234.5`), `formatNumber(n, "percent")` a percentage of a fraction
  (`0.256` is `26%`). All are en-US forms; a non-finite number prints `""`.
- `formatDecimal(cents, 2)` is the exact, ungrouped form (`"1481.47"`) for a field's value.
- Amounts that change in place take `font-variant-numeric="tabular-nums"`.
