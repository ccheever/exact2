# Recipe: theme: colours, light and dark, icons

A role (`-exact-secondary-label`) follows light, dark and Increased Contrast with nothing
written. A brand is `app.tamber` beside `app.contract`: tokens that a row names as it names a
role.

```tamber
tamber brand

theme
  AccentColor = light-dark(#c8102e, #ff453a)
  surface.card = light-dark(#f6f1ea, #1f1b17)
  text.muted = light-dark(#6b5e52, #a89a8c)  pair=surface.card
  brand.fill = #c8102e  scheme=fixed
  ink.on-brand = #ffffff  scheme=fixed

icons
  icon.play = icon(sf "play.fill")
```

```contract
component App
  action dark
    setScheme("dark")
  action system
    setScheme("system")
  view
    column padding=16 gap=12
      column background-color="surface.card" padding=12 gap=4 border-radius=12 testId="card"
        text "Tonight" font="-exact-headline"
        text "3 patterns" color="text.muted"
      row background-color="brand.fill" color="ink.on-brand" padding=12 gap=8 border-radius=12
        image "symbol:icon.play" aria-label="Play"
        text "Now playing"
      row gap=8
        button press=dark testId="dark"
          text "Dark"
        button press=system testId="system"
          text "System"
      column color-scheme="dark" background-color="surface.card" padding=12 testId="always-dark"
        text "Always dark"
```

- **A role if the platform has one, a token if it is the brand** (`color="ink"`, or
  `color="var(--ink)"`; `tamber import css tokens.css` writes a stylesheet's). `contract vocab
  -exact-label` answers a role. From data, a `fn` names the token:
  `` fn tint(t: string): string = `type-${t}` ``.
- **`light-dark(light, dark)`** gives a token both schemes. One colour on purpose (an ink on
  the brand's fill, a ground under media) says `scheme=fixed`; an unmarked one is warned once,
  at its line (`lower-colour-one-scheme`). A root `theme` block in `app.contract` cannot mark
  it: move it to `app.tamber`.
- **`AccentColor`** is the app's tint: filled buttons, a plain button's text, the focus ring.
  A filled button's label stays white, so keep both sides at 3:1 under white: a pale dark
  side is warned (`tamber-contrast`).
- **Icons** are an `icons` block: `sf` is what Apple draws; add `svg "icons/play.svg"`, a
  one-colour file under `assets/`, for the other hosts. The web host every app has needs it
  (`tamber-icon-missing`); with no SVG, write the role if there is one (`symbol:play-fill`, drawn
  everywhere), else `symbol:sf/<name>`, blank on the web. A row draws `image "symbol:icon.play"`,
  coloured by `color` and sized by its font ([symbols](symbols.md)).
- **The scheme:** `setScheme("light" | "dark" | "system")` sets the app's (in its action, and a
  `mount` task for a kept one); `color-scheme`
  fixes a subtree in one. Modes the person picks (Dim, Black) are axes ([tamber](tamber.md)).
- **Check it:** `bun exact.mjs tamber check .` (`pair=` warns below 4.5:1); a drive or an
  authored test says `prefer prefers-color-scheme dark` (then `light`) before an `expect` or
  `screenshot`, or `check both` judges both schemes.
