# Recipe: symbols

`image "symbol:<role>"` is an icon each host draws from its own set (SF Symbols on Apple,
matching outlines elsewhere). `image "symbol:sf/<name>"` is any SF Symbol by Apple's name.

```contract
component Toolbar
  state starred = false
  action star
    starred = not starred
  view
    row gap=16 padding=16 align-items="center"
      button aria-label="Add" testId="add"
        image "symbol:add"
      button press=star aria-label=(starred ? "Unstar" : "Star") testId="star"
        image (starred ? "symbol:star-fill" : "symbol:star")
      button aria-label="Share" testId="share"
        image "symbol:share"
      image "symbol:sf/cloud.sun.rain" aria-label="Showers later" width=28 height=28
```

The roles: `add`, `close`, `back`, `forward-chevron`, `compose`, `pencil`, `delete`,
`search`, `filter`, `sort`, `more`, `ellipsis`, `share`, `copy`, `link`, `pin`,
`bookmark`, `heart`, `star` (each of these three with `-fill`), `checkmark`, `select`,
`circle`, `minus`, `minus-circle`, `alert`, `info`, `lock`, `settings`, `home`,
`person`, `people`, `follow`, `sign-out`, `messages`, `reply`, `send`, `quote`, `at`,
`hashtag`, `notifications`, `calendar`, `chart`, `activity`, `book`, `document`,
`layers`, `photo`, `camera`, `microphone`, `waveform`, `speaker`, `speaker-mute`,
`play`, `pause`, `stop`, `skip-back-15`, `skip-forward-15`, `repeat`, `reorder`,
`sparkles`, `moon`, and more `-fill`s (`home-fill`, `person-fill`, `play-fill`…). An
unknown role is refused with the full list.

- A role draws on every host. `symbol:sf/<name>` (`sf/arrow.clockwise`,
  `sf/cloud.sun.rain`) draws on Apple only, and is blank on the web and Linux; a tab or
  button that must show everywhere takes a role.
- A symbol-only button needs an `aria-label`. A tab is a symbol over its text.
- Leave a symbol's colour unsaid: in a button it is the tint, and it follows dark mode.
  Emoji aren't icons: they ignore the tint, the weight and the theme.
- `width` and `height` size a symbol that stands alone.
