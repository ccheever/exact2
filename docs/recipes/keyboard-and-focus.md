# Recipe: the keyboard and focus

Keep typing or dismiss the keyboard.

```contract
routes nav
  home "/"

shape Guest
  id: string
  name: string

component Guests
  state draft = ""
  state adding = false
  state guests = [Guest(id="1", name="Ada Lovelace"), Guest(id="2", name="Grace Hopper")]
  action began
    adding = true
  action ended
    adding = false
  action done
    blur("draft")
  action edit(v: string)
    draft = v
  action add
    if draft.trim() != ""
      guests = guests.concat([Guest(id=(guests.length + 1).toString(), name=draft.trim())])
      draft = ""
    focus("draft")
  action startAdding
    focus("draft")
  view
    main navigationKey=`${top(nav).id}` testId="app" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="guests" position="absolute" inset=0
          header display="flex" align-items="center" padding="8px 16px"
            text "Guests" role="heading" aria-level=1 flex=1
            if adding
              button press=done testId="done"
                text "Done"
            else
              button press=startAdding aria-label="Add Guest" testId="start-adding"
                image "symbol:add"
          list id="guests" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                input id="draft" value=draft input=edit submit=add focus=began blur=ended
                  placeholder="Add a guest" aria-label="New guest" testId="draft" flex=1 min-width=0
            section
              for g in guests key=g.id
                row
                  text g.name testId=`guest-${g.id}`
```

```contract-test
test "Return adds and keeps typing; Done puts the keyboard away"
  tap "start-adding"
  expect state adding == true
  type "draft" "Alan Turing"
  type "draft" key "Enter"
  expect text "guest-3" == "Alan Turing"
  tap "done"
  expect state adding == false
```

- **Return** calls `submit`; `enterkeyhint` labels the key. iOS dismisses the keyboard;
  `next` moves to the next field. Call `focus("draft")` in its submit action to keep typing.
- **`blur()`** puts the keyboard away from an action, `blur("draft")` from that field only;
  **`focus("id")`** moves it to a field by its `id` (a sheet's first field: `autofocus=true`).
- **iOS Back:** The keyboard travels with its screen; `blur()` dismisses it. A returning editor can claim focus; custom animator setup may withdraw an early claim.
- **`focus=` and `blur=`** say when a field is being edited: here the bar shows Done meanwhile.
- **Search** lives in the bar: `contract vocab search`. Native iOS Back commits its edit and blur before removing the route.
- Put live results above the editor.
- **On scroll,** a drag on a `list` or `scroll` that reaches the keyboard pulls it down (a flick
  that just reaches it ends in a frame, as in UIKit); `keyboardDismissMode="on-drag"` puts it
  away as the drag starts, `"none"` keeps it up and a background tap still dismisses it.
  Composer: [chat-thread](chat-thread.md).
