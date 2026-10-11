# Recipe: a navigation stack

A list that pushes a detail screen, a sheet with Cancel and Save, and a confirmation.
On iOS each route's `header` is its navigation bar, a pushed route gets the system Back
button and edge swipe, a sheet swipes down, and the alert is `UIAlertController`. The
four blocks below are one app; each route block fills its `when` in the first.

<!-- check: app -->
```contract
routes nav
  notes "/"
    note "/note/:id"
    add "/add"

shape Note
  id: string
  title: string

shape Notes
  notes: list<Note>

shape Ack
  ok: bool

component App
  state draft = ""
  state submitted = false
  state aimed = ""
  resource lib = loadNotes() as shape Notes else empty()
  mutation changed as shape Ack queue refreshes lib
  derive titleError = trim(draft) == "" ? "Enter a title." : ""

  action open(id: string)
    nav = push(nav, path("note", id))
  action openAdd
    draft = ""
    submitted = false
    nav = push(nav, "/add")
  action back
    nav = back(nav)
  action follow(url: string)
    nav = go(nav, url)
  action edit(v: string)
    draft = v
  action save
    submitted = true
    if titleError == ""
      send changed = addNote(trim(draft))
      nav = back(nav)
  action askRemove(id: string)
    aimed = id
    showModal("remove")
  action remove
    if top(nav).name == "note"
      nav = back(nav)
    send changed = removeNote(aimed)

  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        when e.name == "notes"
          // the list
        when e.name == "note"
          // the detail
        when e.name == "add"
          // the sheet
      dialog id="remove" role="alertdialog" aria-label="Delete Note?"
        text "This can’t be undone."
        button press=remove destructive=true commandfor="remove" command="close" testId="remove-confirm"
          text "Delete"
        button commandfor="remove" command="close" testId="remove-cancel"
          text "Cancel"
```

The list: a large title (`aria-level=1`), a bar button, rows that push. A row button's
`text` is its title and a trailing `forward-chevron` its disclosure.

<!-- check: route notes -->
```contract
column navigationKey=`${e.id}` navigationScroll="notes-list" testId="route-notes"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
    text "Notes" role="heading" aria-level=1
    button press=openAdd aria-label="Add Note" testId="add-note"
      image "symbol:add"
  list id="notes-list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    section
      each n in lib.notes key=n.id
        button press=open(n.id) testId=`note-${n.id}`
          text n.title
          image "symbol:forward-chevron"
```

The detail: a level-2 heading is the inline title. Route parameters are strings.

<!-- check: route note -->
```contract
column navigationKey=`${e.id}` navigationScroll="note-list" testId="route-note"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" align-items="center" padding="8px 16px"
    text "Note" role="heading" aria-level=2
  list id="note-list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    each n in filter(lib.notes, x => x.id == e.params.id) key=n.id
      section
        row
          text "Title"
          text n.title testId="detail-title"
      section
        button press=askRemove(n.id) destructive=true testId="delete-note"
          text "Delete Note"
```

The sheet: `navigationPresentation="modal"`. Its Cancel is the control whose `id` is the
root's `navigationBack`, so the swipe down presses it too.

<!-- check: route add -->
```contract
column navigationKey=`${e.id}` navigationPresentation="modal" navigationScroll="add-form" testId="route-add"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
    button id="back" press=back testId="add-cancel"
      text "Cancel"
    text "New Note" role="heading"
    button press=save testId="add-save"
      text "Save"
  list id="add-form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    section
      row
        input appearance="none" value=draft input=edit placeholder="Title" aria-label="Title" autofocus=true testId="add-title" flex=1
      footer
        text (submitted ? titleError : "") testId="error-title"
```

```contract-test
test "add validates, saves, opens and deletes after asking"
  tap "add-note"
  clock settle
  tap "add-save"
  expect text "error-title" == "Enter a title."
  type "add-title" "Milk"
  tap "add-save"
  clock settle
  clock data
  tap "Milk"
  clock settle
  expect text "detail-title" == "Milk"
  tap "delete-note"
  clock settle
  tap "remove-confirm"
  clock settle
  clock data
  expect state lib.notes == []
```

- Every route is `position="absolute" inset=0`, a direct child of the root (through
  `each`/`when` only), and covered routes stay mounted: two screens never share a
  `testId`. Build locations with `path("note", id)`, never a template string.
- The alert holds only text, buttons that close it and one Cancel (a closing button
  with no `press`). Open it from an action with `showModal("remove")`. For a modal you
  lay out yourself, use `role="dialog" aria-modal=true`.
- Show a validation error only after the first Save (`submitted`), in the section's
  footer, whose text is always the system's grey.
- In tests, `clock settle` after every push, sheet or alert, and `clock data` before
  reading what a save wrote. Swipe to delete and long-press menus: [list-rows](list-rows.md).
