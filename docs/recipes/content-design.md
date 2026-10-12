# Recipe: content design

- **Chrome is the platform's** (bars, sheets, alerts, tabs, a form's controls: unsaid, they
  look native and follow dark mode); **the content is yours:** rows, grouping, where
  actions sit, the layout. The screen's purpose gets a focal point: large type, a fill
  (the accent or a system colour), a shape, room.
- **Emphasis is a part, not paint on a control.** A `background-color`, `border` or
  `font-size` on a `button` makes it your own box. The parts keep a button's
  semantics, a 44-point target and legible ink.
- **A wide window gets a layout:** two columns or cards (`flex="1 1 320px"` in a wrapping
  row), not a phone's column centred.
- **A status is a `Badge`, not coloured text:** "Full" or "Cancelled" in a capsule beside
  the title, so it reads without its colour. **An action that can't be taken is disabled,
  with its reason:** `disabled=full` and a secondary line under it ("This event is full.";
  in a grouped list, the section's `footer`), never left enabled to refuse after the tap.
  `pattern list-detail` shows both.
- **Colour is a role, a system colour or the accent** (`accent-color` on the root), never
  a hex; a card's surface pairs with its page ([start-here](../start-here.md)).

| Part | Props (`none` for an option) | For |
| --- | --- | --- |
| `RoundButton` | `label`, `tint: option<string>`, `press`, `disabled`, `testId` | Start, Stop, Lap: an 80-point circle, `none` grey; a light hue (green, mint, teal, cyan, yellow, orange) takes black ink |
| `AnswerTile` | `label`, `status`, `press`, `testId` | a choice: `"idle"`, `"selected"`, `"correct"`, `"wrong"`, `"dimmed"` |
| `SummaryCard` | `label`, `value`, `detail`, `progress: option<number>`, `symbol: option<string>` | the screen's one number, leading it |
| `RingProgress` | `label`, `value`, `fraction`, `tint: option<string>` | one fraction as a ring around its number |
| `StatBar` | `label`, `value`, `fraction`, `tint: option<string>` | one of several numbers that compare |
| `RowText` | `title`, `detail` (`""` for none), `value: option<string>` | a row: title, line under it, value |
| `Badge` | `label`, `tint: option<string>` | a status: `none` grey, `some("-exact-system-orange")` for Full |
| `DoneMark` | `caption` (a weekday: it shows), `done` | a habit's day: check or circle |
| `EmptyState` | `symbol`, `title`, `message`; your `button` under it | nothing yet, and the next step |

A timer: round buttons at the ends of a row under the readout.

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
    column gap=32 padding=24
      text "00:12.40" font-size="4.5rem" font-variant-numeric="tabular-nums" text-align="center"
        -exact-fit="shrink" white-space="nowrap" testId="readout"
      row justify-content="space-between"
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
      text "Which planet is the largest?" role="heading" aria-level=2
      each a in ["Mars", "Jupiter", "Venus"] key=a
        AnswerTile(label=a, status=(picked == "" ? "idle" : a == "Jupiter" ? "correct" : a == picked ? "wrong" : "dimmed"), press=pick(a), testId=`answer-${a}`)
```

A dashboard: the hero leads; a wide window puts the breakdown beside it.

```contract
use SummaryCard, StatBar from "exact:content"

component Budget
  view
    scroll height="100%"
      row flex-wrap="wrap" align-items="flex-start" gap=24 padding=16
        column flex="1 1 320px" min-width=0
          SummaryCard(label="Spent", value="$794.90", detail="$5.10 left", progress=some(0.99), symbol=some("symbol:chart"))
        column flex="1 1 320px" min-width=0 gap=14
          text "By Category" role="heading" aria-level=3
          StatBar(label="Groceries", value="$412 of $450", fraction=0.92, tint=none)
          StatBar(label="Eating Out", value="$286 of $200", fraction=1.43, tint=some("-exact-system-red"))
```

`pattern dashboard` is this over a data module, which adds and sorts (Contract has no
`reduce` or `sort`); a readout's size: [numeric-readout](numeric-readout.md).

A history: a row a day, its bar against the goal, a badge when met: in a column on a
wide window, under the bar on a phone.

```contract
use StatBar, Badge from "exact:content"

shape Day
  label: string
  ml: number

component History
  state goal = 2000
  resource days = loadWeek() as shape list<Day> else empty()
  view
    column gap=14 padding=16 max-width=720
      text "Last 7 Days" role="heading" aria-level=3
      each d in days key=d.label
        row flex-wrap="wrap" align-items="center" column-gap=16 row-gap=6
          column flex="1 1 300px" min-width=0
            StatBar(label=d.label, value=`${d.ml} of ${goal} ml`, fraction=d.ml / goal, tint=none)
          column flex="0 0 80px"
            when d.ml >= goal
              Badge(label="Goal met", tint=some("-exact-system-green"))
```
