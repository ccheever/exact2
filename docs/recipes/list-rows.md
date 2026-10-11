# Recipe: list rows: search, filter, swipe and long-press

A list under a large title with the bar's search field, a segmented filter, rows that
swipe to delete and a long-press menu shared by every row.

```contract
routes nav
  home "/"

shape Note
  id: string
  title: string
  pinned: bool

component Notes
  state query = ""
  state scope = "all"
  state aimed = ""
  state notes = [Note(id="1", title="Groceries", pinned=true), Note(id="2", title="Ideas", pinned=false)]
  derive shown = filter(notes, n => (scope == "all" or n.pinned) and includes(toLowerCase(n.title), toLowerCase(trim(query))))
  action search(v: string)
    query = v
  action show(s: string)
    scope = s
  action aim(id: string)
    aimed = id
  action remove(id: string)
    notes = filter(notes, n => n.id != id)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="notes-list" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" flex-wrap="wrap" align-items="center" gap=8 padding="8px 16px"
            text "Notes" role="heading" aria-level=1 flex=1
            input type="search" value=query input=search placeholder="Search" aria-label="Search notes"
              testId="search" width="100%"
          scroll id="notes-list" flex=1 min-height=0
            row role="tablist" aria-label="Filter" margin="8px 16px"
              button role="tab" aria-selected=(scope == "all") press=show("all") testId="filter-all" flex=1
                text "All"
              button role="tab" aria-selected=(scope == "pinned") press=show("pinned") testId="filter-pinned" flex=1
                text "Pinned"
            each n in shown key=n.id
              scroll swipeContent=`note-${n.id}` swipeTrailing=`delete-${n.id}`
                width="100%" overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
                row width="100%"
                  button id=`note-${n.id}` contextmenu=aim(n.id) contextPopover="note-menu" testId=`note-${n.id}`
                    width="100%" flex-shrink=0 scroll-snap-align="start" text-align="start" padding="12px 16px"
                    text n.title
                  button id=`delete-${n.id}` destructive=true press=remove(n.id) aria-label="Delete" testId=`delete-${n.id}`
                    flex-shrink=0 scroll-snap-align="start"
                    image "symbol:delete"
          column id="note-menu" popover="auto" role="menu" aria-label="Note"
            button press=remove(aimed) destructive=true popovertarget="note-menu" popovertargetaction="hide" testId="menu-delete"
              image "symbol:delete"
              text "Delete"
```

```contract-test
test "search, filter, swipe and the long-press menu"
  type "search" "gro"
  expect tree missing "note-2"
  type "search" ""
  tap "filter-pinned"
  expect tree missing "note-2"
  tap "filter-all"
  tap "delete-2"
  expect tree missing "note-2"
  tap "note-1" contextmenu
  tap "menu-delete"
  expect state notes == []
```

- In the `header`, an `input type="search"` is the bar's search field, under the
  large title. A `tablist` in the content (text tabs, no `aria-controls`) is a
  segmented control.
- The swipe is a horizontal snap scroll naming its content and its trailing action; on
  iOS the action is UIKit's own swipe action, and a full swipe performs the first one.
- In a grouped list (`list appearance="auto"`) the same scroll is a section's row, and
  on iOS it swipes as its list cell, as Settings' and Mail's rows do. The section
  already insets the row, so leave the scroll's `width` unsaid there and give it
  `padding-right=0` (a scroll's padding is inside its port, where the action would show
  at rest on the web); the content keeps the row's trailing inset:

  ```text
  scroll swipeContent=`row-${n.id}` swipeTrailing=`delete-${n.id}` padding-right=0
    overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
    row id=`row-${n.id}` width="100%" flex-shrink=0 scroll-snap-align="start"
      padding-right=16 box-sizing="border-box"
      text n.title
    button id=`delete-${n.id}` destructive=true press=remove(n.id) aria-label="Delete"
      flex-shrink=0 scroll-snap-align="start"
      image "symbol:delete"
  ```
- `contextPopover` names one menu every row shares, and the row's own `contextmenu`
  action runs first, so the menu acts on `aimed`. On iOS the menu is `UIMenu`.
- A symbol-only button needs an `aria-label`; a destructive one says `destructive=true`.
