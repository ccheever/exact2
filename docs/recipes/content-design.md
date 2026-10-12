# Recipe: content design

- **Chrome is the platform's** (bars, sheets, alerts, tabs, a form's controls: unsaid, native,
  dark-aware); **the content is yours:** rows, grouping, where actions sit. The screen's
  purpose gets a focal point: large type, a fill (the accent or a system colour), room.
- **Emphasis is a part, not paint on a control.** A `background-color`, `border` or
  `font-size` on a `button` makes it your own box; the parts keep a button's semantics, a
  44-point target and legible ink. `contract vocab <Part>` gives each one's props:
  `RoundButton`, `AnswerTile`, `SummaryCard`, `RingProgress`, `StatBar`, `RowText`, `Badge`,
  `DoneMark`, `EmptyState`.
- **A status is a `Badge`** ("Full" beside the title). **An action that can't be taken is
  disabled with its reason** under it (`disabled=full`; in a grouped list, the section's
  `footer`), never left enabled to refuse after the tap.
- **A wide window gets a layout:** two columns or cards (`flex="1 1 320px"` in a wrapping row).

A timer: round buttons at the row's ends. The column centres the readout, so the row says
`align-self="stretch"`; else it shrinks to its content and bunches them.

```contract
use RoundButton from "exact:content"

component Stopwatch
  state running = false
  state laps = 0
  action toggle
    running = not running
  action lapOrReset
    laps = running ? laps + 1 : 0
  view
    column align-items="center" gap=32 padding=24
      text "00:12.40" font-size="4.5rem" font-variant-numeric="tabular-nums"
        -exact-fit="shrink" white-space="nowrap" testId="readout"
      row align-self="stretch" justify-content="space-between"
        RoundButton(label=(running ? "Lap" : "Reset"), tint=none, press=lapOrReset, disabled=(not running and laps == 0), testId="lap")
        RoundButton(label=(running ? "Stop" : "Start"), tint=some(running ? "-exact-system-red" : "-exact-system-green"), press=toggle, disabled=false, testId="start")
```

A quiz: one tile an answer, told in place.

```contract
use AnswerTile from "exact:content"

component Quiz
  state picked = ""
  action pick(a: string)
    picked = a
  view
    column gap=12 padding=16
      each a in ["Mars", "Jupiter", "Venus"] key=a
        AnswerTile(label=a, status=(picked == "" ? "idle" : a == "Jupiter" ? "correct" : a == picked ? "wrong" : "dimmed"), press=pick(a), testId=`answer-${a}`)
```

A dashboard: the hero leads; a wide window puts the breakdown beside it (`pattern dashboard`).

```contract
use SummaryCard, StatBar from "exact:content"

component Budget
  view
    scroll height="100%"
      row flex-wrap="wrap" align-items="flex-start" gap=24 padding=16
        column flex="1 1 320px" min-width=0
          SummaryCard(label="Spent", value="$794.90", detail="$5.10 left", progress=some(0.99), symbol=some("symbol:chart"))
        column flex="1 1 320px" min-width=0 gap=14
          StatBar(label="Groceries", value="$412 of $450", fraction=0.92, tint=none)
          StatBar(label="Eating Out", value="$286 of $200", fraction=1.43, tint=some("-exact-system-red"))
```
