# Recipe: content design

- **Chrome is the platform's** (bars, sheets, alerts, tabs, a form's controls: unsaid, native,
  dark-aware); **the content is yours:** rows, grouping, where actions sit. The screen's
  purpose gets a focal point: large type, a fill (the accent or a system colour), room.
- **Emphasis is a part, not paint on a control.** A `background-color`, `border` or
  `font-size` on a `button` makes it your own box; the parts keep a button's semantics, a
  44-point target and legible ink. `contract vocab <Part>` gives each one's props and a call to paste:
  `RoundButton`, `AnswerTile`, `SummaryCard`, `RingProgress`, `StatBar`, `RowText`, `Badge`,
  `DoneMark`, `EmptyState`. `RoundButton` is a circle for a word (about 7 characters fit): a main
  action of more words ("Start quiz") is a `button buttonStyle="filled"`; the compiler warns.
- **A status is a `Badge`** ("Full" beside the title). **An action that can't be taken is
  disabled with its reason** under it (`disabled=full`; in a grouped list, the section's
  `footer`), never left enabled to refuse after the tap.
- **A wide window gets the composition the content wants.** One number and its actions (a
  tracker, a timer): a focused column, centred, with a `max-width`. A list and its detail, or a
  dashboard of cards (the hero leads, `pattern dashboard`): two columns (`flex="1 1 320px"` in a
  wrapping row).
- **A value is shown once:** a `SummaryCard`'s number with a second, larger copy beside it is a slip.

A timer is one number and its buttons: a column of `max-width=360`, centred, so on a wide window
they stay a phone's width apart. The column centres the readout, so the row says
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
    column align-items="center" gap=32 padding=24 max-width=360 margin-left="auto" margin-right="auto"
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
