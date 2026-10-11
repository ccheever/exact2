# Recipe: money

Keep money as a whole number of cents, and format it at the edge. Contract has no
currency formatter yet, so `money` below groups thousands by hand: `$1,481.47`, not
`$1481.47`.

<!-- formatNumber: when `formatNumber(n, "currency", "USD")` lands (LLP 1116 D8), replace
`grouped` and `money` below with it, and `formatNumber(n, "decimal")` / `"percent"` in
the rules. -->

```contract
fn pad3(n: number): string = n < 10 ? `00${n}` : n < 100 ? `0${n}` : `${n}`
fn grouped(n: number): string = n < 1000 ? `${n}` : n < 1000000 ? `${floor(n / 1000)},${pad3(n % 1000)}` : `${floor(n / 1000000)},${pad3(floor(n / 1000) % 1000)},${pad3(n % 1000)}`
fn dollars(cents: number): string = `$${grouped(floor(cents / 100))}.${slice(formatDecimal(cents % 100, 2), -2)}`
fn money(cents: number): string = cents < 0 ? `−${dollars(0 - cents)}` : dollars(cents)

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
      row role="tablist" aria-label="Tip"
        each p in [15, 18, 20] key=p
          button role="tab" aria-selected=(tipPercent == p) press=setTip(p) testId=`tip-${p}` flex=1
            text `${p}%`
```

```contract-test
test "money is grouped and rounded to the cent"
  expect text "tip" == "$266.66"
  expect text "total" == "$1,748.13"
  tap "tip-20"
  expect text "total" == "$1,777.76"
```

- Store and send cents (`round(dollars * 100)` once, where the text is parsed): adding
  binary fractions drifts (`0.1 + 0.2`). Round each computed amount once, to a cent.
- `formatDecimal(cents, 2)` is exact (`"1481.47"`) but ungrouped; `toFixed(n, 2)`
  rounds a binary value (`toFixed(1.005, 2)` is `"1.00"`). Neither adds a symbol or
  commas, so a total in a list or a readout goes through `money`.
- `grouped` covers amounts under a billion. Another currency changes the symbol; a
  locale's own format (`1.481,47 €`) is the data module's: `new Intl.NumberFormat(locale,
  { style: "currency", currency: "EUR" }).format(n)` in a source.
- Amounts that change in place take `font-variant-numeric="tabular-nums"`.
