# Recipe: the web look

What an app leaves unsaid, the web draws as a well-made web app would (LLP 1116 D1), and iOS
and macOS draw their own controls from the same file. Say what a thing is; leave the look
alone. A `background-color`, `border`, `border-radius` or `font-size` on a `button` makes it
your own box.

## What the default gives

| You write | The web draws |
|---|---|
| `button` (or `buttonStyle="bordered"`, `"gray"`) | a neutral fill, the accent as its ink, 8 px corners, weight 500, hover and pressed fills |
| `buttonStyle="bordered-prominent"` (or `"filled"`) | the accent as its fill: the screen's one primary action |
| `buttonStyle="plain"`, `"tinted"` | the accent as text only; the accent as a light tint |
| `destructive=true` | the system red, in whatever style the button has |
| `row role="tablist"` in the content | a segmented control: a track, the selected tab raised |
| a root `tablist` (tabs with `aria-controls`) | a tab bar, the selected tab in the accent |
| a route's `header` | an app bar: a material, a separator, sticky on a page that scrolls |
| `dialog role="alertdialog"` | the system alert: its `aria-label` the title, its actions side by side |
| `input type="checkbox" switch` | a drawn switch (Safari's own where it has one) |
| `input` (text, `date`, `time`, `datetime-local`), `textarea`, `select` | 36 px fields with a 1 px line and 8 px corners; a date or time keeps the browser's picker |
| `text role="heading" aria-level=1…3` | the level's text style, bold (`headline` from level 4, semibold) |

What you write wins: a heading's `font-weight`, an `accent-color` on a button or the root.
The one colour to write is that accent, a role or one `light-dark(light, dark)` pair. A
page, card or text colour is a role (`-exact-secondary-grouped-background`,
`-exact-label`) or unsaid: a hex stays the same in dark mode while the controls change.

## The attributes that matter

- `buttonStyle="bordered-prominent"` once a screen, for the action people came for.
- `destructive=true` on anything that deletes; ask first with a `dialog role="alertdialog"`.
- `list appearance="auto" listStyle="inset-grouped"` of `section`s for a settings screen.
- **Use a wide window.** A list and its detail sit side by side when the window is wide,
  and a phone pushes the detail, as below. A phone-width column centred on a desktop is
  "a stretched phone UI".

## List and detail: one column on a phone, side by side when wide

```contract
routes nav
  people "/"
    person "/person/:id"

shape Person
  id: string
  name: string
  phone: string

shape Viewport
  width: number

component People
  resource people = loadPeople() as shape list<Person> else empty()
  resource viewport = exactViewport() as shape Viewport
  derive wide = viewport.width >= 720
  derive selected = top(nav).name == "person" ? top(nav).params.id : ""
  action open(id: string)
    nav = selected == "" ? push(nav, path("person", id)) : replace(nav, path("person", id))
  action back
    nav = back(nav)
  view
    main navigationKey=(wide ? "split" : `${top(nav).id}`) navigationBack="back" testId="app" width="100%" height="100%"
      when wide
        column navigationKey="split" navigationScroll="side" position="absolute" inset=0 display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "People" role="heading" aria-level=1
          row flex=1 min-height=0
            list id="side" appearance="auto" listStyle="plain" width=320 flex-shrink=0
              Rows(people=people, selected=selected, pushes=false, open=open)
            scroll flex=1 min-width=0 border-left-width=1 border-left-style="solid" border-color="-exact-separator"
              each p in filter(people, x => x.id == selected) key=p.id
                Detail(p=p)
              when selected == ""
                text "Choose a person." color="-exact-secondary-label" padding=24
      else
        each e in stack(nav) key=e.id
          column navigationKey=`${e.id}` navigationScroll=`content-${e.id}` position="absolute" inset=0
            display="flex" flex-direction="column"
            header display="flex" align-items="center" padding="8px 16px"
              text (e.name == "people" ? "People" : "") role="heading" aria-level=(e.name == "people" ? 1 : 2)
            when e.name == "people"
              list id=`content-${e.id}` appearance="auto" listStyle="plain" flex=1 min-height=0
                Rows(people=people, selected="", pushes=true, open=open)
            else
              scroll id=`content-${e.id}` flex=1 min-height=0
                each p in filter(people, x => x.id == e.params.id) key=p.id
                  Detail(p=p)

component Rows
  props
    people: list<Person>
    selected: string
    pushes: bool
    open: action
  view
    section
      each p in people key=p.id
        button press=open(p.id) aria-current=(p.id == selected) testId=`person-${p.id}`
          text p.name font=(p.id == selected ? "-exact-headline" : "-exact-body")
            color=(p.id == selected ? "AccentColor" : "-exact-label")
          when pushes
            image "symbol:forward-chevron"

component Detail
  props
    p: Person
  view
    column gap=4 padding=24
      text p.name role="heading" aria-level=2
      text "Phone" font="-exact-subheadline" color="-exact-secondary-label" margin-top=12
      link href=`tel:${p.phone}` testId="detail-phone"
        text p.phone
```

```ts
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.people'; // exact new writes this line
export const grants = '';

const sources: Sources = {
  loadPeople: () => [
    { id: '1', name: 'Ada Lovelace', phone: '+1 415 555 0101' },
    { id: '2', name: 'Grace Hopper', phone: '+1 415 555 0102' },
  ],
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "a wide window shows the list beside the detail"
  size 1000x700
  tap "person-2"
  expect text "detail-phone" == "+1 415 555 0102"
  tap "person-1"
  expect text "detail-phone" == "+1 415 555 0101"

test "a phone pushes the detail"
  size 390x844
  tap "person-2"
  clock settle
  expect text "detail-phone" == "+1 415 555 0102"
```

- `exactViewport()` is the window: `when wide` chooses the composition, not only the
  wrap. Both read the router, so a selection survives a resize: a wide window replaces
  the selected route, a phone pushes it.
- Wide, it is one route with one app bar: the list keeps its width (`width=320
  flex-shrink=0`), the detail takes the rest, and the selected row is the accent, not a
  painted box. A phone's rows push, so they carry the chevron.
- Columns that wrap rather than change: [responsive-columns](responsive-columns.md).
