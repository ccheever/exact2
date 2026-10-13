# Recipe: columns that wrap on a phone

A board of columns side by side on a wide window, stacked on a phone. Write it as CSS
would: wrapping flex items with a minimum width, no measuring.

```contract
shape Card
  id: string
  title: string
  column: string

component Board
  state cards = [Card(id="1", title="Write the brief", column="todo"), Card(id="2", title="Review the plan", column="doing")]
  view
    scroll height="100%" testId="board"
      row flex-wrap="wrap" align-items="flex-start" gap=16 padding=16
        for col in ["todo", "doing", "done"] key=col
          column flex="1 1 280px" min-width=0 gap=8 testId=`column-${col}`
            text (col == "todo" ? "To Do" : col == "doing" ? "Doing" : "Done") role="heading" aria-level=3
            for c in filter(cards, x => x.column == col) key=c.id
              text c.title padding=12 border-radius=10 background-color="-exact-secondary-background" testId=`card-${c.id}`
            if length(filter(cards, x => x.column == col)) == 0
              text "No cards" color="-exact-secondary-label" padding=12
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
  shows one per line. For a different layout, read the `exactViewport` resource's `width`.
- A list beside its detail when wide, pushed on a phone: `pattern list-detail`.
- A horizontal board on a phone is a `scroll overflow-x="scroll"` of fixed-width
  columns instead; never three columns squeezed into a third of a phone each.
- Look at both sizes: `agent web --size 390x844 "screenshot .exact/phone.png"` and
  `--size 1000x700`. A test's `size` line does the same.

## Paging columns

A masonry has no virtualized collection edges: `reachstart` and `reachend` are
refused. Put one `intersect=` marker after the columns, inside their scroller:

```contract
component Feed
  state cards = [0, 1, 2]
  action loadMore(entry: IntersectionObserverEntry)
    if entry.isIntersecting and length(cards) < 12
      let next = length(cards)
      cards = concat(cards, [next, next + 1, next + 2])
  view
    scroll height="100%" testId="feed"
      row align-items="flex-start" gap=16
        for col in [0, 1, 2] key=col
          column flex=1 min-width=0 gap=8
            for card in filter(cards, n => n % 3 == col) key=card
              text `Card ${card + 1}` padding=16 testId=`card-${card}`
      if length(cards) < 12
        for boundary in [length(cards)] key=boundary
          view intersect=loadMore intersect-margin="0px 0px 800px 0px"
            height=1 testId="load-more"
```

Ignore leave notifications; remount the keyed marker after appending to fill a
short page. Remove it at the end. A server feed also guards against a pending
request and changes the key only after appending its answer.
[Delivery rules](../reference.md#intersection-visibility).
