# Recipe: content design

The platform draws the controls, the bars and the type ramp; the app's own content is
yours to design. A content screen is not another settings list (inset-grouped is for
settings and forms): lead with a summary, give rows a hierarchy, let colour mean
something, put bars where numbers compare, and make an empty list say what to do.

<!-- exact:content: the same screen with `use SummaryCard, StatBar, EmptyState, RowText from "exact:content"` -->

```contract
shape Category
  id: string
  name: string
  count: number
  spent: number
  budget: number

shape Summary
  spent: number
  budget: number
  categories: list<Category>

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
        column gap=8 padding=20 border-radius=20 background-color="-exact-secondary-background"
          text "Spent" font="-exact-subheadline" color="-exact-secondary-label"
          text money(summary.spent) font="-exact-large-title" font-variant-numeric="tabular-nums" testId="spent"
          progress value=min(summary.spent, summary.budget) max=summary.budget width="100%" aria-label="Budget used"
            accent-color=(left < 0 ? "-exact-system-red" : "-exact-system-green")
          text (left < 0 ? `${money(-left)} over budget` : `${money(left)} left of ${money(summary.budget)}`)
            color=(left < 0 ? "-exact-system-red" : "-exact-secondary-label") testId="left"
        when length(summary.categories) == 0
          column align-items="center" gap=8 padding="32px 24px" testId="empty"
            image "symbol:chart" width=40 height=40
            text "Nothing spent this week" role="heading" aria-level=3
            text "Expenses show here by category as you add them." color="-exact-secondary-label" text-align="center"
            button press=show("month") testId="see-month"
              text "See This Month"
        else
          text "By Category" role="heading" aria-level=3
          each c in summary.categories key=c.id
            column gap=8 padding=16 border-radius=16 background-color="-exact-secondary-background" testId=`category-${c.id}`
              row align-items="center" gap=12
                column flex=1 min-width=0 gap=2
                  text c.name font="-exact-headline"
                  text (c.count == 1 ? "1 expense" : `${c.count} expenses`) font="-exact-subheadline" color="-exact-secondary-label"
                text money(c.spent) font-variant-numeric="tabular-nums" color=(c.spent > c.budget ? "-exact-system-red" : "-exact-label")
              progress value=min(c.spent, c.budget) max=c.budget width="100%" aria-label=`${c.name}: ${money(c.spent)} of ${money(c.budget)}`
                accent-color=(c.spent > c.budget ? "-exact-system-red" : "AccentColor")
```

The data module adds and groups (Contract has no `reduce` or `sort`); the view shows:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.budget'; // exact new writes this line
export const grants = '';

const month: Result<'loadSummary'>['categories'] = [
  { id: 'groceries', name: 'Groceries', count: 9, spent: 412.5, budget: 450 },
  { id: 'dining', name: 'Eating Out', count: 7, spent: 286, budget: 200 },
  { id: 'transport', name: 'Transport', count: 12, spent: 96.4, budget: 150 },
];
const sources: Sources = {
  loadSummary: ([period]) => period === 'week' ? { spent: 0, budget: 200, categories: [] }
    : { spent: month.reduce((sum, c) => sum + c.spent, 0), budget: 800, categories: month },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "the month leads with its total; an empty week says what to do"
  expect text "spent" == "$794.90"
  expect text "left" == "$5.10 left of $800.00"
  tap "week"
  clock data
  tap "see-month"
  clock data
  expect tree has "category-dining"
```

- **The summary**: the screen's one number in a tinted card (`-exact-secondary-background`),
  a text style with `tabular-nums`, its context in `-exact-secondary-label`.
- **Rows**: a `-exact-headline` title over `-exact-subheadline` secondary details, the
  value trailing. **Colour** means something: red over budget, green under.
- **Bars**: `progress value= max=` is the platform's own bar, tinted by `accent-color`.
- **Empty**: say why, and offer the next step as a button.
- Big type is a text style (`-exact-large-title`, `-exact-title1`); a literal `font-size`
  only for a readout's digits ([numeric-readout](numeric-readout.md)).
