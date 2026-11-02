# Recipe: the web look

What an app leaves unsaid, the web draws as a well-made web app would; iOS and macOS
draw their own controls from the same file. Say what a thing is; leave the look alone: a
`background-color`, `border`, `border-radius` or `font-size` on a `button` makes it your own
box (emphasis is a part: [content-design](content-design.md)).

| You write | The web draws |
|---|---|
| `button`; `buttonStyle="gray"` | an outline, a wash on hover and press; a grey fill |
| `"bordered-prominent"` | the accent as its fill: the action people came for |
| `"plain"`, `"tinted"`; `destructive=true` | the accent as text, as a tint; red |
| a pressable card, a `link` | a wash on hover, a dim while pressed |
| `row role="radiogroup"` of `button role="radio"`; a root `tablist` | a segmented control; a tab bar |
| a route's `header`: one heading, buttons | an app bar; a pushed route's Back is an arrow and "Back" |
| `header appearance="none"`; a site's: a `nav`, or two links not in a menu | either as written |
| a sheet's `header` of a heading and buttons | a dialog: the title on top, the buttons as words at its foot (Cancel, then Save); a `close` after the title its × |
| `dialog role="alertdialog"` | an alert, its `aria-label` the title |
| a checkbox, radio, range; `switch` | in the accent; a switch (in a sheet, a checkbox) |
| `input`, `select`, a date or time | 36 px fields (44 on touch); the browser's picker |
| `listStyle="inset-grouped"` | edged cards on the page, 720 px wide at most |
| a row's `forward-chevron` | nothing |

What you write wins: a heading's `font-weight`, an `accent-color` on a button or the root.
The one colour to write is that accent, a role or a `light-dark(light, dark)` pair; a page,
card or text colour is a role (a card on a plain page `-exact-secondary-background`) or
unsaid: a hex stays the same in dark mode while the controls change.

- `destructive=true` on anything that deletes, asked first with a `dialog
  role="alertdialog"`. An icon is `image "symbol:<role>"` ([symbols](symbols.md)), never a
  character (▦ ☏ ★) or an emoji.
- **Compose for a wide window by what the content is.** One number and its actions: a
  centred column, `max-width` a phone's width or a little more. A list and its detail, or a
  dashboard of cards: two columns. A root tab bar is a sidebar and a list whose rows push
  (`forward-chevron`) sits beside its detail, with nothing to write ([tabs](tabs.md),
  [navigation-stack](navigation-stack.md)); a layout chosen by width (`exactViewport()`):
  [responsive-columns](responsive-columns.md).
- **A route is a page.** The tab's title is the route `header`'s heading, focus moves to
  it on a push and back on Back, a sheet is a dialog to a screen reader, a tab bar and a
  menu take arrow keys: nothing to write. A row that only navigates is a `link
  href=path(…)`, not a `button`, so a new tab and "Copy Link" work
  ([navigation-stack](navigation-stack.md)).
