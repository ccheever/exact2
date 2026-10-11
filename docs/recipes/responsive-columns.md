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
        each col in ["todo", "doing", "done"] key=col
          column flex="1 1 280px" min-width=0 gap=8 testId=`column-${col}`
            text (col == "todo" ? "To Do" : col == "doing" ? "Doing" : "Done") role="heading" aria-level=3
            each c in filter(cards, x => x.column == col) key=c.id
              text c.title padding=12 border-radius=10 background-color="-exact-secondary-background" testId=`card-${c.id}`
            when length(filter(cards, x => x.column == col)) == 0
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

- `flex="1 1 280px"` on each column of a `flex-wrap="wrap"` row: as many 280-point
  columns as fit share the width, and a phone shows one per line. A grid does the same:
  `display="grid" grid-template-columns="repeat(auto-fill, minmax(280px, 1fr))"`.
- To choose a different layout, not just a different wrap, read the window:
  `resource viewport = exactViewport() as shape Viewport` (`width: number`) and
  `when viewport.width >= 700`.
- A horizontal board on a phone is a `scroll overflow-x="scroll"` of fixed-width
  columns instead; never three columns squeezed into a third of a phone each.
- Look at both sizes: `agent web --size 390x844 "screenshot .exact/phone.png"` and
  `--size 1000x700`. A test's `size` line does the same.
