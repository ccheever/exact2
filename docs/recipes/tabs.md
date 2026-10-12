# Recipe: tabs

A tab bar is for an app with at least two top-level sections; a one-tab bar is a tell. Each tab
keeps its own stack of routes, nested under it in the routes table.

```contract
routes nav
  tab today "/"
  tab history "/history"
    day "/day/:key"

component App
  action pick(name: string)
    nav = select(nav, name)
  action follow(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="app"
      width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 min-height=0 position="relative"
        each t in nav.tabs key=t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
            each e in t.stack key=e.id
              column navigationKey=`${e.id}` navigationScroll=`content-${e.id}` position="absolute" inset=0
                header padding="8px 16px"
                  text (e.name == "today" ? "Today" : e.name == "history" ? "History" : e.params.key)
                    role="heading" aria-level=(e.name == "day" ? 2 : 1)
                scroll id=`content-${e.id}` flex=1 min-height=0
                  when e.name == "history"
                    link href=path("day", "2026-10-09") testId="day-2026-10-09"
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

- The root `tablist` (each tab a symbol over a label, naming its panel with `aria-controls`) is the
  iOS tab bar. A value chosen from two to five is a `row role="radiogroup"` of `button
  role="radio" aria-checked=…` instead, the segmented control ([persisted-setting](persisted-setting.md)).
- Every tab's stack stays mounted: a pushed screen, a draft and a scroll offset survive a visit to
  another tab; selecting the shown tab again pops it to its root.
- A row that only navigates is a `link`, which the root's `navigate` handler (`follow`) pushes
  onto the selected tab's stack; the browser's Back, Forward and a deep link arrive there too.
- Hide the bar on one screen with `display="none"` on the tablist, never `when`: the panels'
  routes are then found by no host.
- A wide window (900 px up on the web) and a Mac window draw the bar, placed as above (the root
  column's last child), as a sidebar; a tablist you arrange yourself stays yours.
