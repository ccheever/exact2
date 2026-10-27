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
      image "symbol:sf/cloud.sun.rain" aria-label="Showers later" font="-exact-title1"
      row gap=6 align-items="center"
        image "symbol:calendar" aria-hidden=true
        text "Tomorrow"
```

The roles: `add`, `close`, `back`, `forward-chevron`, `chooser`, `compose`, `pencil`, `delete`,
`search`, `filter` (narrowing a list: Apple's decreasing lines), `adjust` (sliders), `sort`,
`more`, `ellipsis`, `menu` (a drawer or sidebar), `share`, `copy`, `link`, `pin`, `refresh`,
`download`, `download-pending`, `install`, `open-app`, `package`, `mail`, `emoji`, `translate`,
`bookmark`, `heart`, `star` (each with `-fill`), `checkmark`, `select`,
`circle`, `minus`, `minus-circle`, `alert`, `info` (each with `-fill`), `lock`, `settings`, `home`,
`person`, `people`, `follow`, `sign-out`, `messages`, `reply`, `send`, `quote`, `at`,
`hashtag`, `notifications`, `calendar`, `chart`, `activity`, `book`, `document`,
`layers`, `photo`, `camera`, `microphone`, `waveform`, `speaker`, `speaker-mute`,
`play`, `pause`, `stop`, `skip-back-15`, `skip-forward-15`, `repeat`, `reorder` (a drag handle),
`sparkles`, `moon`, and more `-fill`s (`home-fill`, `play-fill`…). Unknown literals fail the build; computed sources are checked at runtime.
`contract vocab symbol` lists roles. A role also takes SF Symbols names and aliases (`gearshape`, `gear`, `trash`, `chevron-right`, `checkmark.circle.fill`): it builds as
the role, with a warning that names it (`lower-symbol-alias`).

- A role draws everywhere; `sf/<name>` on Apple only. Else add an icon token to
  `app.tamber`'s `icons`, `icon.rain = icon(sf "cloud.rain", svg "icons/rain.svg")` (a
  one-colour SVG), and write `symbol:icon.rain`.
- A symbol-only button needs an `aria-label`. A tab is a symbol over its text. A symbol beside
  text that says the same is decoration: `aria-hidden=true`, so VoiceOver reads it once.
- `activity` is an ECG trace (heart, workouts). Done or not done is `DoneMark` (`exact:content`).
- Leave a symbol's colour unsaid: in a button it is the tint, and it follows dark mode.
  A `color` written on the symbol is its colour, as `currentcolor` paints an icon on the
  web; a row's trailing `forward-chevron` is already the platform's grey.
  Emoji aren't icons: they ignore the tint, the weight and the theme.
- A symbol takes its text's size. Use `font="-exact-title1"` to follow the reader's
  text size; `width` does not size its glyph.
