# Recipe: tabs

Use tabs for at least two top-level sections, each with its own route stack. A one-tab bar is a tell.

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

- The root `tablist` (each tab a symbol over a label, naming its panel with `aria-controls`)
  is the iOS tab bar. For a segmented value picker, use a `radiogroup`
  ([persisted-setting](persisted-setting.md)).
- Stacks stay mounted: pushed screens, drafts and scroll offsets survive tab switches.
  Selecting the shown tab again pops it to its root.
- A `link` uses the root's `navigate` handler to push onto the selected stack; browser
  Back, Forward and deep links arrive there too.
- Hide the bar with `display="none"`, never `when`: removing the tablist loses the panels' routes.
- A Mac window, or a web window 900 px up, draws the root column's last-child tablist as a
  sidebar; a tablist you arrange yourself stays yours.

`appearance="none"` on a `header` or root `tablist` keeps the authored bar on every host.
On a header button it keeps only that button authored (the Mac keeps the whole header).
Styling alone does not opt out. See [the reference](../reference.md).
