# Recipe: the keyboard and focus

What Return does, putting the keyboard away, and moving the focus: said on the field and
in actions, done by each host with its own keyboard.

```contract
shape Guest
  id: string
  name: string

component Guests
  state query = ""
  state draft = ""
  state searching = false
  state guests = [Guest(id="1", name="Ada Lovelace"), Guest(id="2", name="Grace Hopper")]
  derive shown = filter(guests, g => includes(toLowerCase(g.name), toLowerCase(trim(query))))
  action search(v: string)
    query = v
  action began
    searching = true
  action ended
    searching = false
  action searched
    blur()
  action cancel
    query = ""
    blur("search")
  action edit(v: string)
    draft = v
  action add
    if trim(draft) != ""
      guests = concat(guests, [Guest(id=toString(length(guests) + 1), name=trim(draft))])
      draft = ""
  action startAdding
    focus("draft")
  view
    column height="100%" display="flex" flex-direction="column" testId="app"
      row gap=8 padding="8px 16px" align-items="center"
        input id="search" type="search" value=query input=search submit=searched focus=began blur=ended
          placeholder="Search" aria-label="Search guests" enterkeyhint="search" testId="search" flex=1 min-width=0
        when searching or query != ""
          button press=cancel testId="cancel"
            text "Cancel"
        else
          button press=startAdding aria-label="Add Guest" testId="start-adding"
            image "symbol:add"
      list appearance="auto" listStyle="inset-grouped" keyboardDismissMode="interactive" flex=1 min-height=0
        section
          row
            input id="draft" appearance="none" value=draft input=edit submit=add placeholder="Add a guest"
              aria-label="New guest" autocomplete="name" testId="draft" flex=1 min-width=0
        section
          each g in shown key=g.id
            row
              text g.name testId=`guest-${g.id}`
```

```contract-test
test "Return searches and puts the keyboard away; Cancel clears it"
  type "search" "ada"
  expect state searching == true
  type "search" key "Enter"
  expect state searching == false
  expect tree missing "guest-2"
  type "search" "grace"
  tap "cancel"
  expect state query == ""
  expect state searching == false

test "Return in the add field adds and keeps typing"
  tap "start-adding"
  type "draft" "Katherine Johnson"
  type "draft" key "Enter"
  expect text "guest-3" == "Katherine Johnson"
  expect state draft == ""
```

- **Return** is the field's `submit`; `enterkeyhint` labels the key. On iOS `done`,
  `go`, `search` and `send` also put the keyboard away, and `next` moves to the next
  field (at the last, puts it away). No hint keeps the keyboard up, for typing several
  in a row, as the add field does. On the Mac Return also ends the editing as a text
  field's does: the field keeps the focus with its text selected, unless the action
  moved it. There Space presses the focused control, and Return a `dialog`'s default
  button: its one `buttonStyle="filled"` button that is not `destructive`, when there
  is exactly one (a field with `submit` takes Return itself; one without passes it on).
- **`blur()`** puts the keyboard away from an action on every host (the web keeps the
  focus on Enter): call it where the typing is finished, as `searched` does.
  `blur("search")` takes the focus from that field only.
- **`focus("id")`** moves the focus to a field by its `id`, from an action; a sheet's
  first field takes `autofocus=true` instead. On the Mac a new window opens with the
  caret in its first text field, as AppKit's windows do (its `focus=` runs then);
  `autofocus=true` on another control starts there instead.
- **`focus=` and `blur=`** on a field say when it is being edited. Keep Cancel while
  there is a query too: on the web, pressing it blurs the field first. On the Mac a
  click on a button leaves the focus in the field, as AppKit's buttons do, so say
  `blur()` or `focus("id")` in the action when the press should move it.
- **Tab** moves between the fields. On the Mac it reaches the buttons too only where the
  person has turned Keyboard navigation on in System Settings, as in every Mac app; a
  `tabindex=0` you write makes a control a stop whatever the setting.
- **On scroll:** `keyboardDismissMode="interactive"` on the `list` or `scroll` lets a drag
  pull the keyboard down, as Messages does; `"on-drag"` puts it away as the drag starts;
  without it the keyboard stays, as UIKit's default. A composer that rides above the
  keyboard: [chat-thread](chat-thread.md).
