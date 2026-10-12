# Recipe: the web look

What an app leaves unsaid, the web draws as a well-made web app would, and iOS and macOS
draw their own controls from the same file. Say what a thing is; leave the look alone: a
`background-color`, `border`, `border-radius` or `font-size` on a `button` makes it your own
box (emphasis is a part: [content-design](content-design.md)).

| You write | The web draws |
|---|---|
| `button` (or `buttonStyle="bordered"`, `"gray"`) | a neutral fill, the accent's ink, 8 px corners, hover and pressed fills |
| `buttonStyle="bordered-prominent"` (`"filled"`) | the accent as its fill: the screen's one primary action |
| `buttonStyle="plain"`, `"tinted"` | the accent as text; as a light tint |
| `destructive=true` | the system red, in the button's style |
| a card with a `press`, a custom `button`, a `link` | dims while pressed, and from Space; `-exact-press-scale` replaces the dim; a disabled custom `button` is dimmed |
| `row role="radiogroup"` of `button role="radio"` (a value from two to five); a root `tablist` (`aria-controls`) | a segmented control, the arrows moving the choice; a tab bar |
| a route's `header` | an app bar, sticky on a page that scrolls |
| `dialog role="alertdialog"` | the system alert: its `aria-label` the title |
| `input type="checkbox" switch` | a switch |
| `input`, `textarea`, `select`, a `date` or `time` | 36 px fields; the browser's picker |
| `text role="heading" aria-level=1…3` | the level's text style, bold |

What you write wins: a heading's `font-weight`, an `accent-color` on a button or the root.
The one colour to write is that accent, a role or one `light-dark(light, dark)` pair; a
page, card or text colour is a role (a card on a plain page `-exact-secondary-background`,
text `-exact-label`) or unsaid: a hex stays the same in dark mode while the controls change.

- `buttonStyle="bordered-prominent"` once a screen, for the action people came for;
  `destructive=true` on anything that deletes, asked first with a `dialog
  role="alertdialog"`; `list appearance="auto" listStyle="inset-grouped"` of `section`s
  for settings.
- **Use a wide window:** a phone's column centred on a desktop is "a stretched phone UI".
  A root tab bar is a sidebar there, and a list whose rows push (the `forward-chevron`)
  sits beside its detail, with nothing to write ([tabs](tabs.md), [navigation-stack](navigation-stack.md)).
  For another composition, `resource viewport = exactViewport() as shape Viewport`
  (`width: number`) and `when viewport.width >= 900`. Cards that wrap: [responsive-columns](responsive-columns.md).
- **A route is a page.** The tab's title is the route `header`'s heading (a `head`'s
  `title` wins), the focus moves to that heading on a push and back to the row on Back,
  a sheet is a dialog to a screen reader, and a tab bar and a menu move by arrow keys,
  with nothing to write. A row that only navigates is a `link href=path(…)`, not a `button`:
  a new tab, "Copy Link" and the status bar's address work too ([navigation-stack](navigation-stack.md)).
