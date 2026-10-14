# Recipe: a numeric readout

A timer's or a total's big number is deliberate design: it may set its size. Give it
`font-variant-numeric="tabular-nums"`, so every digit is the same width and the number
doesn't jitter as it changes, and `-exact-fit="shrink"` with `white-space="nowrap"`, so it
stays on its line at the largest text sizes.

```contract
component Water
  state ml = 750
  state goal = 2000
  action drink(amount: number)
    ml = ml + amount
  view
    column align-items="center" gap=4 padding=24
      text `${ml}` font-size="4.5rem" font-variant-numeric="tabular-nums" -exact-fit="shrink" white-space="nowrap"
        aria-label=`${ml} of ${goal} ml` testId="total"
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
- `-exact-fit="shrink"` draws a line too wide for its box smaller until it fits, as
  UIKit's readouts do (`adjustsFontSizeToFitWidth`): down to half its size, or a display
  size down to the reader's text size. Without it, a stopwatch's `00:48.12` at the
  largest accessibility size ran off both edges (only `:48.` showed). It never caps the
  size: a line that fits keeps it. Every host lays it out at the same scale.
- Only the number's size is designed. Its colour is unsaid or the accent
  (`color="AccentColor"`), never a hex, which stays the same in dark mode; its label
  stays the platform's (`color="-exact-secondary-label"`), and a title is a heading,
  never a big `font-size`.
- Columns of numbers (laps, a ledger) take `tabular-nums` at body size too, so they
  line up.
- Give the readout an `aria-label` that says what the number is; VoiceOver reads the
  label, not the digits alone.
