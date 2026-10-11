# Recipe: content design

`bun exact.mjs pattern dashboard` lays this out, working and tested; read on to add the
parts to a screen you have. The platform draws the controls, bars and type; the content
is yours. `exact:content` draws its parts in the platform's roles and the app's accent
(`accent-color` on the root). Every prop is required: `none` for an option.

| Part | Props | For |
| --- | --- | --- |
| `SummaryCard` | `label`, `value`, `detail`, `progress: option<number>`, `symbol: option<string>` | the screen's one number, leading it |
| `RingProgress` | its comment in `<exact2>/contract/lib/content.contract` | one fraction as a ring around its number: a timer, a day's goal |
| `StatBar` | `label`, `value`, `fraction: number`, `tint: option<string>` | one of several numbers that compare |
| `RowText` | `title`, `detail` (`""` for none), `value: option<string>` | a row's title over a secondary line, the value trailing |
| `EmptyState` | `symbol`, `title`, `message`; your `button` indented under it | nothing to show yet: why, and the next step |

```contract
use SummaryCard, StatBar, EmptyState from "exact:content"

shape Category
  id: string
  name: string
  spent: number
  budget: number

shape Summary
  spent: number
  budget: number
  categories: list<Category>

fn money(n: number): string = formatNumber(n, "currency", "USD")

component Budget
  resource summary = loadSummary() as shape Summary else empty()
  derive left = summary.budget - summary.spent
  view
    scroll height="100%" testId="app"
      column gap=16 padding=16
        SummaryCard(label="Spent", value=money(summary.spent), detail=`${money(left)} left`, progress=some(summary.spent / summary.budget), symbol=some("symbol:chart"))
        when length(summary.categories) == 0
          EmptyState(symbol="symbol:chart", title="Nothing spent yet", message="Expenses show here by category as you add them.")
        else
          text "By Category" role="heading" aria-level=2
          each c in summary.categories key=c.id
            StatBar(label=c.name, value=`${money(c.spent)} of ${money(c.budget)}`, fraction=c.spent / c.budget, tint=(c.spent > c.budget ? some("-exact-system-red") : none))
```

- **Lead with the summary**: the screen's one number in tabular figures, its context
  beside it, a `progress` or a ring for how far along it is.
- **Colour means something.** The accent is the ordinary state; a second colour is the
  other: `-exact-system-green` done or under, `-exact-system-red` over or failed,
  `-exact-secondary-label` not yet. Pass it as a part's `tint`.
- **Bars where numbers compare**, a ring for one fraction; never a drawn track.
- **Rows** are records, not a settings list: inset-grouped is for settings and forms.
- The data module adds, groups and sorts (Contract has no `reduce` or `sort`); the view
  shows.
- Big type is a text style (`-exact-large-title`); a literal `font-size` only on a
  readout's digits ([numeric-readout](numeric-readout.md)). Wide on the web: two
  columns ([responsive-columns](responsive-columns.md)).
