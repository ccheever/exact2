# Recipe: a navigation stack

To receive a complete custom-scheme URL, add `protocol_handlers` to `app.json`:
`[{"protocol":"web+notes","url":"/open?u=%s"}]`. Add an `/open` route;
the navigation root's `navigate=` handler receives `/open?u=…` while running.
At launch that location is the router's first visit. Read the original URL,
including its scheme and fragment, with `searchParam(top(nav), "u")`.

A list that pushes a detail screen, a sheet with Cancel and Save, and a confirmation.
On iOS each route's `header` is its navigation bar, a pushed route gets the system Back
button and edge swipe, a sheet swipes down, and the alert is `UIAlertController`. On the
web the browser's Back pops the stack, a reload keeps the place and each screen has a
link. A second screen is always a route: screens swapped by a `when` over a state get
none of that, and warn (`lower-screen-by-state`). On the
Mac a pushed route gets Go ▸ Back (⌘[), a back chevron in the window's toolbar and the
trackpad's swipe back; each presses a Back control the route declares, as iOS's do; the
chevron stays in the toolbar, disabled, at the stack's root, so the title bar does not
change height as routes come and go; the alert is `NSAlert`, a sheet on the window. The
contract blocks below are one app; each route block fills its `when` in the first.

A `header` is the bar when it is the route's first child and holds exactly one heading
(`text role="heading"`): the heading is the title (large at `aria-level=1`), buttons
before it lead and after it trail, the first text after it is the subtitle, and the rest
of the header is hidden. With no heading or two the header is no bar and draws in the
page. Each route's bar is its own: a stack whose first route has none still shows a
later route's, and a pushed route with no bar of its own (no header, or one that is no
bar) gets a bar holding the system Back button alone. A full-bleed screen that draws
its own way back says `navigationBar="none"` on its route: no bar, and the edge swipe
still goes back. `contract vocab header` states it whole, and the compiler warns
(`lower-header-no-bar`, `lower-header-unplaced`, and `lower-back-twice` for a Back
control drawn under the bar's). The bar's title names the screen: a
heading at the top of the content that says it again shows it twice (`lower-title-twice`).

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

shape Reply
  ok: bool
  error: string

component App
  state draft = ""
  state submitted = false
  state aimed = ""
  resource lib = loadNotes() as shape Notes else empty()
  mutation added as shape Reply refreshes lib then afterSave
  mutation removed as shape Reply refreshes lib
  derive titleError = trim(draft) == "" ? "Enter a title." : ""
  derive saveError = match added { case some(r) => r.error, case none => "" }
  derive formError = not submitted ? "" : titleError != "" ? titleError : pending(added) ? "" : saveError

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
    if titleError == "" and not pending(added)
      send added = addNote(trim(draft))
  action afterSave
    if saveError == "" and top(nav).name == "add"
      nav = back(nav)
  action askRemove(id: string)
    aimed = id
    showModal("remove")
  action remove
    if top(nav).name == "note"
      nav = back(nav)
    send removed = removeNote(aimed)

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

The list: a large title (`aria-level=1`), a bar button, rows that push. A row that only
navigates is a `link` to its route (`href=path(…)`): `follow` pushes it on every host, and
on the web it is a real link (a new tab, "Copy Link"). A row's `text` is its title and a
trailing `forward-chevron` its disclosure. A row that does more than navigate (`openAdd`
clears the draft first) stays a `button` with a `press`.

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
        link href=path("note", n.id) testId=`note-${n.id}`
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
        column flex=1 min-width=0 gap=4
          input value=draft input=edit placeholder="Title" aria-label="Title" autofocus=true testId="add-title"
          when formError != ""
            text formError font="-exact-footnote" color="-exact-system-red" testId="error-title"
```

The data module refuses a second note of the same title, in words the sheet shows:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.notes'; // exact new writes this line
export const grants = '';

let notes: Result<'loadNotes'>['notes'] = [], next = 1;
const sources: Sources = {
  loadNotes: () => ({ notes }),
  addNote: ([title]) => {
    const same = notes.find((n) => n.title.toLowerCase() === title.toLowerCase());
    if (same) return { ok: false, error: `There’s already a note called ${same.title}. Choose another title.` };
    notes = [...notes, { id: String(next++), title }];
    return { ok: true, error: '' };
  },
  removeNote: ([id]) => { notes = notes.filter((n) => n.id !== id); return { ok: true, error: '' }; },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "add validates, saves, refuses a repeat, opens and deletes after asking"
  tap "add-note"
  clock settle
  tap "add-save"
  expect text "error-title" == "Enter a title."
  type "add-title" "Milk"
  tap "add-save"
  clock settle
  clock data
  tap "add-note"
  clock settle
  type "add-title" "milk"
  tap "add-save"
  clock data
  expect text "error-title" == "There’s already a note called Milk. Choose another title."
  back
  clock settle
  tap "Milk"
  clock settle
  expect text "detail-title" == "Milk"
  back
  clock settle
  tap "Milk"
  clock settle
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
  with no `press`). Open it from an action with `showModal("remove")`.
- A modal you lay out yourself (a form, an editor) is a sheet, as `add` is: open it
  with `push`, close it with `back`. On iOS it is UIKit's sheet, which swipes down; on
  the web Escape presses its Cancel, and on every host the page under it is inert.
  `role="dialog"` on a box gets none of that (`lower-dialog-role`). A `dialog` you lay
  out, opened with `showModal(id)`, is a smaller thing: the web's modal `dialog`, and on
  iOS a sheet as tall as its content, with no bar and no route.
- Show a validation error only after the first Save (`submitted`), in red under the
  field, in its row. The sheet closes in `then`, only when
  the save's reply is ok: a refusal keeps the sheet, its text and the reason
  ([after-a-write](after-a-write.md)).
- In tests, `clock settle` after every push, sheet or alert, and `clock data` before
  reading what a save wrote. `back` is the platform's Back: it cancels the alert,
  dismisses the sheet (pressing its Cancel) and pops the detail, on every host.
  Swipe to delete and long-press menus: [list-rows](list-rows.md).
- A wide window (900 px and up on the web) shows the list beside its detail with nothing
  to write, as a split view does: its rows push, because they draw the disclosure (the
  `forward-chevron`). A sheet stays a sheet; a phone pushes. A row pressed while a detail
  shows pushes another, so to swap the detail instead, `follow` replaces when one is shown:
  `nav = top(nav).name == "note" and startsWith(url, "/note/") ? replace(nav, url) : go(nav, url)`.
  `navigationSplit="none"` on the list's route keeps it pushed; `contract vocab navigationSplit`.
  Until a row is chosen the detail pane says "No Selection", as an iPad's does. For your own, mark a box
  in the list's route `navigationSplit="placeholder"` (an `EmptyState`): it shows there then, and nowhere else.
- On the web a route is a page: the tab's title is the route's heading, the focus moves to
  it on a push and back to the row on Back, and the browser's Back and Forward follow the
  stack.
