# Recipe: a navigation stack

A list that pushes a detail, a Cancel/Save sheet and an alert (`pattern list-detail`).
A second screen is a route (`if` over state gets no Back, links or split). Each route block
fills its `if`.

A route's `header` is its bar when first in the route with one heading (the title: large at
`aria-level=1`, inline at 2); buttons before it lead, after it trail. A pushed route without
one gets the system Back (`contract vocab header`).

<!-- check: app -->
```contract
routes nav
  notes "/"
    note "/note/:id"
    add "/add"

shape Note
  id: string
  title: string

shape Reply
  ok: bool
  error: string

component App
  state draft = ""
  resource lib = loadNotes() as shape Note[] else empty()
  mutation added as shape Reply refreshes lib then afterSave
  mutation removed as shape Reply refreshes lib
  derive saveError = match added { case some(r) => r.error, case none => "" }

  action openAdd
    nav = push(nav, "/add")
  action back
    nav = back(nav)
  action follow(url: string)
    nav = go(nav, url)
  action edit(v: string)
    draft = v
  action save
    if draft.trim() != "" and not pending(added)
      send added = addNote(draft.trim())
  action afterSave
    if saveError == "" and top(nav).name == "add"
      draft = ""
      nav = back(nav)
  action askRemove
    showModal("remove")
  action remove
    send removed = removeNote(top(nav).params.id)
    nav = back(nav)

  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
      for e in stack(nav) key=e.id
        if e.name == "notes"
          // the list
        if e.name == "note"
          // the detail
        if e.name == "add"
          // the sheet
      dialog id="remove" role="alertdialog" aria-label="Delete Note?"
        text "This can't be undone."
        button press=remove destructive=true commandfor="remove" command="close" testId="remove-confirm"
          text "Delete"
        button commandfor="remove" command="close"
          text "Cancel"
```

The list: a row that only navigates is a `link` (`href=path(…)`, never a template string)
that `follow` pushes.

<!-- check: route notes -->
```contract
column navigationKey=`${e.id}` navigationScroll="notes-list" position="absolute" inset=0
  header display="flex" align-items="center" padding="8px 16px"
    text "Notes" role="heading" aria-level=1 flex=1
    button press=openAdd aria-label="Add Note" testId="add-note"
      image "symbol:add"
  list id="notes-list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    section
      for n in lib key=n.id
        link href=path("note", n.id) testId=`note-${n.id}`
          text n.title
          image "symbol:forward-chevron"
```

<!-- check: route note -->
```contract
column navigationKey=`${e.id}` navigationScroll="note-list" position="absolute" inset=0
  header display="flex" align-items="center" padding="8px 16px"
    text "Note" role="heading" aria-level=2
  list id="note-list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    for n in lib.filter(x => x.id == e.params.id) key=n.id
      section
        text n.title testId="detail-title"
        button press=askRemove destructive=true testId="delete-note"
          text "Delete Note"
```

The sheet: `navigationPresentation="modal"`; its Cancel's `id` is the root's `navigationBack`,
which the swipe down presses.

<!-- check: route add -->
```contract
column navigationKey=`${e.id}` navigationPresentation="modal" navigationScroll="add-form" position="absolute" inset=0
  header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
    button id="back" press=back
      text "Cancel"
    text "New Note" role="heading"
    button press=save buttonStyle="filled" testId="add-save"
      text "Save"
  list id="add-form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    section
      row
        input value=draft input=edit submit=save placeholder="Title" flex=1 min-width=0 testId="add-title"
```

```contract-test
test "add, open, delete after asking"
  tap "add-note"
  clock settle
  type "add-title" "Milk"
  tap "add-save"
  clock data
  clock settle
  tap "note-1"
  clock settle
  expect text "detail-title" == "Milk"
  tap "delete-note"
  clock settle
  tap "remove-confirm"
  clock data
  expect state lib == []
```

- The alert holds text, closing buttons and one Cancel; any other modal is a sheet (`push`,
  `back`), closed in `then` only on an ok reply ([after-a-write](after-a-write.md)).
- Covered routes stay mounted: no two screens share a `testId`. In tests, `clock settle` after a
  push, sheet or alert; `clock data` before a save's read.
- A wide window (900 px up) shows the list beside its detail when its rows push (a `link`, or a
  `press` that writes the router); `follow` replaces it; `navigationSplit="none"` keeps it pushed.
- A location no route matches keeps its URL and shows not-found with a way home, unless the
  routes declare `notfound`; a detail whose id names nothing says so.
