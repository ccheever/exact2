# Recipe: list rows: search, swipe, filter and long-press

A list under a large title and the bar's search field; its rows swipe to delete and
long-press for a menu.

```contract
routes nav
  home "/"

shape Note
  id: string
  title: string

component Notes
  state query = ""
  state notes = [Note(id="1", title="Groceries"), Note(id="2", title="Ideas")]
  state aimed = ""
  derive shown = notes.filter(n => n.title.toLowerCase().includes(query.trim().toLowerCase()))
  action search(v: string)
    query = v
  action remove(id: string)
    notes = notes.filter(n => n.id != id)
  action aim(id: string)
    aimed = id
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="notes-list" position="absolute" inset=0
          header display="flex" flex-wrap="wrap" align-items="center" gap=8 padding="8px 16px"
            text "Notes" role="heading" aria-level=1 flex=1
            input type="search" value=query input=search placeholder="Search" aria-label="Search notes"
              testId="search" width="100%"
          list id="notes-list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              for n in shown key=n.id
                scroll swipeContent=`row-${n.id}` swipeTrailing=`delete-${n.id}` padding-right=0
                  overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
                  row id=`row-${n.id}` width="100%" flex-shrink=0 scroll-snap-align="start"
                    padding-right=16 box-sizing="border-box" contextmenu=aim(n.id) contextPopover="menu"
                    text n.title testId=`note-${n.id}`
                  button id=`delete-${n.id}` destructive=true press=remove(n.id) aria-label="Delete"
                    flex-shrink=0 scroll-snap-align="start" testId=`delete-${n.id}`
                    image "symbol:delete"
          column id="menu" popover="auto" role="menu"
            button press=remove(aimed) destructive=true popovertarget="menu" popovertargetaction="hide"
              testId="menu-delete"
              image "symbol:delete"
              text "Delete"
```

```contract-test
test "search, swipe to delete, delete from the menu"
  type "search" "gro"
  expect tree missing "note-2"
  type "search" ""
  tap "delete-2"
  expect tree missing "note-2"
  tap "note-1" contextmenu
  tap "menu-delete"
  expect tree missing "note-1"
```

- The swipe is a horizontal snap scroll naming its content and its trailing action (on iOS
  UIKit's own; a full swipe runs it). In a grouped list: no `width` on the scroll,
  `padding-right=0` on it, `padding-right=16` on the content.
- A filter: a `row role="radiogroup"` of `button role="radio"` above the list
  ([persisted-setting](persisted-setting.md)).
- One long-press menu serves every row: the row notes which (`aim`), each item acts on `aimed`
  and hides it (iOS: a `UIMenu`). Test: `tap "note-1" contextmenu`.
