# Recipe: the web look

What an app leaves unsaid, the web draws as a well-made web app would, and iOS and macOS
draw their own controls from the same file. Say what a thing is; leave the look alone: a
`background-color`, `border`, `border-radius` or `font-size` on a `button` makes it your own
box (emphasis is a part: [content-design](content-design.md)).

| You write | The web draws |
|---|---|
| `button` (or `buttonStyle="bordered"`); `"gray"` | an outline, a wash on hover and press; a grey fill |
| `buttonStyle="bordered-prominent"` (`"filled"`) | the accent as its fill: once a screen, the action people came for |
| `buttonStyle="plain"`, `"tinted"` | the accent as text; as a light tint |
| `destructive=true` | red, in the button's style |
| a card with a `press`, a custom `button`, a `link` | a wash on hover; dims while pressed and from Space (an authored press replaces the dim); dimmed when disabled |
| `row role="radiogroup"` of `button role="radio"` (a value from two to five); a root `tablist` (`aria-controls`) | a segmented control, arrows move the choice; a tab bar |
| a route's `header` | an app bar, sticky on a page that scrolls |
| `dialog role="alertdialog"` | an alert: its `aria-label` the title |
| an `input` checkbox, radio, range; `switch` | in the accent; a switch |
| `input`, `textarea`, `select`, a `date` or `time` | 36 px fields; the browser's picker |
| `text role="heading" aria-level=1…3` | the level's text style, bold |

What you write wins: a heading's `font-weight`, an `accent-color` on a button or the root.
The one colour to write is that accent, a role or one `light-dark(light, dark)` pair; a
page, card or text colour is a role (a card on a plain page `-exact-secondary-background`,
text `-exact-label`) or unsaid: a hex stays the same in dark mode while the controls change.

- `destructive=true` on anything that deletes, asked first with a `dialog
  role="alertdialog"`; `list appearance="auto" listStyle="inset-grouped"` of `section`s
  for settings.
- **Compose for a wide window by what the content is.** One number and its actions (a
  tracker, a timer): a focused column, centred, `max-width` a phone's width or a little more;
  stretched to the window, its buttons sit at the two edges with a gap between. A list and its
  detail, or a dashboard of cards: two columns. A root tab bar is a sidebar and a list whose
  rows push (the `forward-chevron`) sits beside its detail, with nothing to write
  ([tabs](tabs.md), [navigation-stack](navigation-stack.md)); wrapping cards and a layout
  chosen by width (`exactViewport()`): [responsive-columns](responsive-columns.md).
- **A route is a page.** The tab's title is the route `header`'s heading (a `head`'s
  `title` wins), focus moves to it on a push and back to the row on Back, a sheet is a dialog
  to a screen reader, a tab bar and a menu take arrow keys: nothing to write. A row that only
  navigates is a `link href=path(…)`, not a `button` (a new tab, "Copy Link" and the status
  bar's address work: [navigation-stack](navigation-stack.md)).
