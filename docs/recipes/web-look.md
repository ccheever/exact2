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
| `input`, `textarea`, `select` | 36 px fields with a 1 px line and 8 px corners |
| `text role="heading" aria-level=1…3` | the level's text style, bold (`headline` from level 4, semibold) |

What you write wins: a heading's `font-weight`, an `accent-color` on a button or the root.

## The attributes that matter

- `buttonStyle="bordered-prominent"` once a screen, for the action people came for.
- `destructive=true` on anything that deletes; ask first with a `dialog role="alertdialog"`.
- `list appearance="auto" listStyle="inset-grouped"` of `section`s for a settings screen.
- On wide screens, a centred column is optional (it changes layout): `column max-width="640px"
  width="100%" box-sizing="border-box" margin="0 auto"`, as below.

```contract
routes nav
  home "/"

component Settings
  state size = "medium"
  state alerts = true
  action back
    nav = back(nav)
  action choose(next: string)
    size = next
  action flip(on: bool)
    alerts = on
  action ask
    showModal("reset")
  action reset
    size = "medium"
    alerts = true
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="content" position="absolute" inset=0 display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Settings" role="heading" aria-level=1 flex=1
          scroll id="content" flex=1 min-height=0
            column gap=16 padding=16 max-width="640px" width="100%" box-sizing="border-box" margin="0 auto"
              row role="tablist" aria-label="Text size"
                button role="tab" aria-selected=(size == "small") press=choose("small") flex=1
                  text "Small"
                button role="tab" aria-selected=(size == "medium") press=choose("medium") flex=1
                  text "Medium"
              row gap=8 align-items="center"
                text "Alerts" flex=1
                input type="checkbox" switch checked=alerts change=flip aria-label="Alerts"
              row gap=8 justify-content="end"
                button press=ask destructive=true
                  text "Reset"
                button press=back buttonStyle="bordered-prominent"
                  text "Done"
          dialog id="reset" role="alertdialog" aria-label="Reset settings?"
            text "Text size and alerts go back to their defaults."
            button press=reset destructive=true commandfor="reset" command="close"
              text "Reset"
            button commandfor="reset" command="close"
              text "Cancel"
```
