# A brand: Light, Dim and Black

An app's own colours are its design, `app.tamber` beside `app.contract` (LLP
1121). Rows name its tokens as they name roles; a mode the person picks (Dim or
Black for dark) is an app axis, set by `setMode` and held by a pin.

```tamber
tamber brand

axes
  dark-style = black | dim  under scheme=dark

theme
  surface.page = light-dark(#ffffff, #000000)
    when dark-style=dim: light-dark(#ffffff, #15202b)
  surface.card = light-dark(#f1f3f5, #16181c)
    when dark-style=dim: light-dark(#f1f3f5, #1e2732)
  text.primary = light-dark(#0f1419, #e7e9ea)
    when dark-style=dim: light-dark(#0f1419, #f7f9f9)
```

```contract
component App
  action dim
    setMode("dark-style", "dim")
  action black
    setMode("dark-style", "black")
  view
    column background-color="surface.page" color="text.primary" padding=16 gap=12
      row gap=8
        button press=dim testId="dim"
          text "Dim"
        button press=black testId="black"
          text "Black"
      column background-color="surface.card" padding=12 testId="card"
        text "Follows the window"
      column -exact-mode="dark-style=black" color-scheme="dark" background-color="surface.card" padding=12 testId="preview"
        text "Always Black"
```

What to know:

- **Names, not values.** `bun exact.mjs tamber check .` checks the design;
  `tamber eval . --mode dark-style=dim` prints each token in that mode;
  `tamber fmt .` gives the file its one layout.
  `no-tells` reads the design: a token is not a literal.
- **An axis is the app's.** `under scheme=dark` keeps the light side; a `when`
  line gives the mode's value; the first value is the default.
- **A pin** (`-exact-mode="axis=value"`) holds its subtree in a mode; inherited
  text inside takes the pin's value. `axis:value` is refused. A pin picks the
  app's mode, not the scheme: a preview that is always dark says
  `color-scheme="dark"` too.
- **Drive it** with `prefer mode dark-style=dim`, after `clock data`: an app
  that restores a kept mode at launch overrides an earlier one. `layout <id>`
  shows the token beside each row's value.
- **One colour on purpose** (white ink on the brand fill): `ink.on-brand =
  #ffffff  scheme=fixed`; an unmarked one-colour token is warned once, at its
  line here.
- **One way to write a colour:** a role if the platform has one, a token if it
  is the brand.
- **Edit it while it runs** (development only): the dev loop serves
  `/__tamber`. `bun exact.mjs tamber op <url> '{"method":"describe"}'` reads;
  a `transaction` names its `client` and the file's digest, and writes a line.
  `{"method":"history"}` reads the design's git log as token edits, and
  `revert` holds a past commit's inverse for review. `bun exact.mjs tamber
  tool .` opens the design tool on it (LLP 1122). The tool always lives at
  `http://127.0.0.1:7740`, so one runs per machine, on one project: to
  switch, stop it with ctrl-c and run `tamber tool <other app>`, adding
  `--via <its URL>` when that app's dev loop is running.
