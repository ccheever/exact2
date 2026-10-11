# Recipe: content design

The platform draws the controls, the bars and the type ramp; the app's own content is
yours to design. A content screen is not another settings list (inset-grouped is for
settings and forms): lead with a summary, give rows a hierarchy, let colour mean
something, put bars where numbers compare, and make an empty list say what to do.

`exact:content` draws those four parts in the platform's roles: `SummaryCard`,
`StatBar`, `EmptyState` (your `button` indented under it) and `RowText`. Every prop is
required; pass `none` for an option.

<!-- exact:content: re-fence this block and its test as `contract` / `contract-test` once the library is on main (LLP 1116.000). -->
```text
use SummaryCard, StatBar, EmptyState, RowText from "exact:content"

shape Category
  id: string
  name: string
  spent: number
  budget: number

shape Expense
  id: string
  title: string
  detail: string
  amount: number

shape Summary
  spent: number
  budget: number
  categories: list<Category>
  recent: list<Expense>

fn money(n: number): string = formatNumber(n, "currency", "USD")

component Budget
  state period = "month"
  resource summary = loadSummary(period) as shape Summary else empty()
  derive left = summary.budget - summary.spent
  derive detail = left < 0 ? `${money(-left)} over budget` : `${money(left)} left of ${money(summary.budget)}`
  action show(p: string)
    period = p
  view
    scroll height="100%" testId="app"
      column gap=16 padding=16
        row role="tablist" aria-label="Period"
          button role="tab" aria-selected=(period == "week") press=show("week") testId="week" flex=1
            text "This Week"
          button role="tab" aria-selected=(period == "month") press=show("month") testId="month" flex=1
            text "This Month"
        SummaryCard(label="Spent", value=money(summary.spent), detail=detail, progress=some(summary.spent / summary.budget), symbol=some("symbol:chart"))
        when length(summary.categories) == 0
          EmptyState(symbol="symbol:chart", title="Nothing spent this week", message="Expenses show here by category as you add them.")
            button press=show("month") testId="see-month"
              text "See This Month"
        else
          text "By Category" role="heading" aria-level=3
          each c in summary.categories key=c.id
            StatBar(label=c.name, value=`${money(c.spent)} of ${money(c.budget)}`, fraction=c.spent / c.budget, tint=(c.spent > c.budget ? some("-exact-system-red") : none))
          text "Recent" role="heading" aria-level=3
          each x in summary.recent key=x.id
            RowText(title=x.title, detail=x.detail, value=some(money(x.amount)))
```

```text
test "the month leads with its total; an empty week says what to do"
  expect state detail == "$5.10 left of $800.00"
  tap "week"
  clock data
  tap "see-month"
  clock data
  expect state summary.recent.0 .title == "Corner Market"
```

The same screen in plain Contract, which is what the components draw (copy and change
it when one doesn't fit):

```contract
shape Category
  id: string
  name: string
  spent: number
  budget: number

shape Expense
  id: string
  title: string
  detail: string
  amount: number

shape Summary
  spent: number
  budget: number
  categories: list<Category>
  recent: list<Expense>

fn money(n: number): string = formatNumber(n, "currency", "USD")

component Budget
  state period = "month"
  resource summary = loadSummary(period) as shape Summary else empty()
  derive left = summary.budget - summary.spent
  action show(p: string)
    period = p
  view
    scroll height="100%" testId="app"
      column gap=16 padding=16
        row role="tablist" aria-label="Period"
          button role="tab" aria-selected=(period == "week") press=show("week") testId="week" flex=1
            text "This Week"
          button role="tab" aria-selected=(period == "month") press=show("month") testId="month" flex=1
            text "This Month"
        column gap=8 padding=20 border-radius=16 background-color="-exact-secondary-background"
          text "Spent" font="-exact-headline"
          text money(summary.spent) font="-exact-large-title" font-variant-numeric="tabular-nums" testId="spent"
          progress value=min(summary.spent, summary.budget) max=summary.budget width="100%" aria-label="Spent"
          text (left < 0 ? `${money(-left)} over budget` : `${money(left)} left of ${money(summary.budget)}`)
            font="-exact-subheadline" color=(left < 0 ? "-exact-system-red" : "-exact-secondary-label") testId="left"
        when length(summary.categories) == 0
          column align-items="center" gap=8 padding="32px 24px" text-align="center" testId="empty"
            image "symbol:chart" width=48 height=48
            text "Nothing spent this week" role="heading" aria-level=2
            text "Expenses show here by category as you add them." color="-exact-secondary-label"
            button press=show("month") testId="see-month"
              text "See This Month"
        else
          text "By Category" role="heading" aria-level=3
          each c in summary.categories key=c.id
            column gap=6 testId=`category-${c.id}`
              row gap=8
                text c.name flex=1 min-width=0
                text `${money(c.spent)} of ${money(c.budget)}` color="-exact-secondary-label" font-variant-numeric="tabular-nums"
              progress value=min(c.spent, c.budget) max=c.budget width="100%" aria-label=c.name
                accent-color=(c.spent > c.budget ? "-exact-system-red" : "AccentColor")
          text "Recent" role="heading" aria-level=3
          each x in summary.recent key=x.id
            row align-items="center" gap=12 padding="10px 0" testId=`expense-${x.id}`
              column flex=1 min-width=0 gap=2
                text x.title
                text x.detail font="-exact-subheadline" color="-exact-secondary-label"
              text money(x.amount) color="-exact-secondary-label" font-variant-numeric="tabular-nums"
```

The data module adds and groups (Contract has no `reduce` or `sort`); the view shows:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.budget'; // exact new writes this line
export const grants = '';

const month: Result<'loadSummary'>['categories'] = [
  { id: 'groceries', name: 'Groceries', spent: 412.5, budget: 450 },
  { id: 'dining', name: 'Eating Out', spent: 286, budget: 200 },
  { id: 'transport', name: 'Transport', spent: 96.4, budget: 150 },
];
const recent = [
  { id: '1', title: 'Corner Market', detail: 'Groceries · Today', amount: 23.4 },
  { id: '2', title: 'Noodle Bar', detail: 'Eating Out · Yesterday', amount: 31 },
];
const sources: Sources = {
  loadSummary: ([period]) => period === 'week' ? { spent: 0, budget: 200, categories: [], recent: [] }
    : { spent: month.reduce((sum, c) => sum + c.spent, 0), budget: 800, categories: month, recent },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "the month leads with its total; an empty week says what to do"
  expect text "spent" == "$794.90"
  expect text "left" == "$5.10 left of $800.00"
  tap "week"
  clock data
  expect tree has "empty"
  tap "see-month"
  clock data
  expect tree has "category-dining"
```

- **The summary**: the screen's one number in a tinted card, in tabular figures, with its
  context in `-exact-secondary-label` and a `progress` for how far along it is.
- **Bars where numbers compare**, in a role colour that means something: the system red
  over budget, the accent otherwise.
- **Rows**: a title over a `-exact-subheadline` secondary line, the value trailing.
- **Empty**: say why, and offer the next step as a button.
- Big type is a text style (`-exact-large-title`); a literal `font-size` only on a
  readout's digits ([numeric-readout](numeric-readout.md)).
