# Recipe: the keyboard and focus

What Return does, putting the keyboard away, and moving the focus: said on the field and in
actions.

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
    focus("draft")
  action startAdding
    focus("draft")
  view
    column height="100%" testId="app"
      row gap=8 padding="8px 16px" align-items="center"
        input id="search" type="search" value=query input=search submit=searched focus=began blur=ended
          placeholder="Search" aria-label="Search guests" enterkeyhint="search" testId="search" flex=1 min-width=0
        when searching or query != ""
          button press=cancel testId="cancel"
            text "Cancel"
        else
          button press=startAdding aria-label="Add Guest" testId="start-adding"
            image "symbol:add"
      list appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
        section
          row
            input id="draft" value=draft input=edit submit=add placeholder="Add a guest"
              aria-label="New guest" testId="draft" flex=1 min-width=0
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
```

- **Return** is the field's `submit`; `enterkeyhint` labels the key. On iOS it puts the keyboard
  away, and `next` moves to the next field. To keep typing several in a row, focus the field
  again in the action, as `add` does: the keyboard stays up and never flickers.
- **`blur()`** puts the keyboard away from an action; `blur("search")` takes the focus from
  that field only. **`focus("id")`** moves it to a field by its `id`; a sheet's first field takes
  `autofocus=true` instead.
- **`focus=` and `blur=`** say when a field is being edited. Keep Cancel while there is a query
  too: on the web, pressing it blurs the field first.
- **On scroll,** a drag on a `list` or `scroll` that reaches the keyboard pulls it down, as
  Messages does; `keyboardDismissMode="on-drag"` puts it away as the drag starts, `"none"` keeps it
  up. A composer above the keyboard: [chat-thread](chat-thread.md).
