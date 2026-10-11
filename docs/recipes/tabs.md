# Recipe: tabs

A tab bar is for an app with at least two top-level sections. A single screen has no
`tablist`: a one-tab bar is a tell. Each tab keeps its own stack of routes, so screens
it pushes nest under it in the routes table.

```contract
routes nav
  tab today "/"
  tab history "/history"
    day "/day/:key"

component App
  action pick(name: string)
    nav = select(nav, name)
  action openDay(key: string)
    nav = push(nav, path("day", key))
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app"
      width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 min-height=0 position="relative"
        each t in nav.tabs key=t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
            each e in t.stack key=e.id
              column navigationKey=`${e.id}` navigationScroll=`content-${e.id}` position="absolute" inset=0
                display="flex" flex-direction="column"
                header display="flex" align-items="center" padding="8px 16px"
                  text (e.name == "today" ? "Today" : e.name == "history" ? "History" : e.params.key)
                    role="heading" aria-level=(e.name == "day" ? 2 : 1)
                scroll id=`content-${e.id}` flex=1 min-height=0
                  when e.name == "history"
                    button press=openDay("2026-10-09") testId="day-2026-10-09"
                      text "Yesterday"
                  else
                    text "Nothing logged yet." color="-exact-secondary-label" padding=16
      row role="tablist" testId="tabs"
        button role="tab" aria-controls="panel-today" aria-selected=(nav.tab == "today") press=pick("today")
          testId="tab-today" flex=1
          image "symbol:calendar"
          text "Today"
        button role="tab" aria-controls="panel-history" aria-selected=(nav.tab == "history") press=pick("history")
          testId="tab-history" flex=1
          image "symbol:chart"
          text "History"
```

```contract-test
test "each tab keeps its own stack"
  tap "tab-history"
  tap "day-2026-10-09"
  clock settle
  expect state nav.tab == "history"
  tap "tab-today"
  tap "tab-history"
  expect tree has "day-2026-10-09"
```

- The root `tablist` (each tab a symbol over a label, naming its panel with
  `aria-controls`) is the iOS tab bar. A `tablist` in the content is a segmented
  control instead ([list-rows](list-rows.md)).
- Every tab's stack stays mounted: a pushed screen, a draft and a scroll offset survive
  a visit to another tab. Selecting the shown tab again pops it to its root.
- To hide the bar on one screen, set `display="none"` on the tablist; never remove it
  with `when`, or the panels' routes are found by no host.
- Under the agent the authored tablist paints; `--chrome platform` and `screenshot …
  window` show UIKit's (put `clock +300 real` before the screenshot).
