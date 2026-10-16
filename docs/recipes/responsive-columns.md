# Recipe: columns that wrap on a phone

Columns side by side when wide, stacked on a phone, and a title and button that
restack at the largest text size: flex items that wrap at a minimum width, as in CSS.

```contract
shape Card
  id: string
  title: string
  column: string

component Board
  state cards = [Card(id="1", title="Write the brief", column="todo"), Card(id="2", title="Review the plan", column="doing")]
  action add()
    cards = cards.concat([Card(id="3", title="New", column="todo")])
  view
    scroll height="100%"
      row flex-wrap="wrap" align-items="center" gap=8 padding=16
        text "Sprint 12" role="heading" flex="1 1 12rem"
        button press=add() buttonStyle="bordered" testId="add-card"
          text "Add card"
      row flex-wrap="wrap" align-items="flex-start" gap=16 padding=16
        for col in ["todo", "doing", "done"] key=col
          column flex="1 1 280px" min-width=0 gap=8 testId=`column-${col}`
            text (col == "todo" ? "To Do" : col == "doing" ? "Doing" : "Done") role="heading" aria-level=3
            for c in cards.filter(x => x.column == col) key=c.id
              text c.title padding=12 border-radius=10 background-color="-exact-secondary-background" testId=`card-${c.id}`
```

```contract-test
test "three columns sit side by side on a wide window"
  size 1000x700
  expect tree has "column-done"

test "they stack on a phone"
  size 390x844
  expect tree has "column-done"
```

- `flex="1 1 280px"`: as many 280-point columns as fit share the width; a phone
  shows one per line. Else read `exactViewport`'s `width`.
- A list beside its detail when wide: `pattern list-detail`.
- Sideways on a phone: a `scroll overflow-x="scroll"` of fixed-width columns, never
  squeezed ones.
- `flex="1 1 12rem"`: a `rem` basis grows with the text size, so at the largest the
  button wraps under the title (`verify ios` reports what spills there).

## Paging columns

A masonry has no collection edges: `reachstart` and `reachend` are
refused. Put one `intersect=` marker after the columns, inside their scroller:

```contract
component Feed
  state cards = [0, 1, 2]
  action loadMore(entry: IntersectionObserverEntry)
    if entry.isIntersecting and cards.length < 12
      let next = cards.length
      cards = cards.concat([next, next + 1, next + 2])
  view
    scroll height="100%"
      row align-items="flex-start" gap=16
        for col in [0, 1, 2] key=col
          column flex=1 min-width=0 gap=8
            for card in cards.filter(n => n % 3 == col) key=card
              text `Card ${card + 1}` padding=16 testId=`card-${card}`
      if cards.length < 12
        for boundary in [cards.length] key=boundary
          view intersect=loadMore intersect-margin="0px 0px 800px 0px"
            height=1 testId="load-more"
```

Ignore leave notifications; remount the keyed marker after appending to fill a
short page. Remove it at the end. A server feed also skips while a request is
pending and rekeys only after appending its answer.
[Delivery rules](../reference.md#intersection-visibility).
