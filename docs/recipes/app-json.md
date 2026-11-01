# Recipe: app.json, assets and modules

`app.json` names the app and what it ships; its `$schema` gives an editor every key.

```json
{
  "$schema": "../exact2/scripts/app.schema.json",
  "app": { "id": "com.yourcompany.euxy", "name": "Euxy" },
  "name": "Euxy",
  "modules": ["waveform-view"],
  "audio_session": "playback",
  "icons": [{ "src": "assets/icon-1024.png", "sizes": "1024x1024", "type": "image/png" }],
  "protocol_handlers": [{ "protocol": "euxy", "url": "/p/%s" }]
}
```

```contract
sound "assets/tick.wav"

component App
  action tick
    playSound("assets/tick.wav")
  view
    column
      image "assets/logo.png" width=96 object-fit="contain" aria-label="Euxy"
      button press=tick testId="tick"
        text "Tick"
```

- **`assets/`** ships whole on every host. Contract names a file by its path from the app
  (no leading slash): `image "assets/logo.png"`, a `sound` and `playSound`, a `font` block's
  `400 = "assets/Inter-Regular.ttf"` (then `font-family="Inter"`).
- **`modules/apple/*.swift`** (or `*.xcframework`) is the app's native module;
  `modules/web/index.js` answers on the web ([native-module](native-module.md)). `modules`
  lists the hyphenated tags of native views ([native-views](native-views.md)): a tag the
  list lacks is `bake-unknown-module`.
- **`audio_session`**: `"playback"` where sound is the point (the silent switch leaves it
  on); the default `ambient` mixes with other audio.
- **`app.id` and `app.name`** are set before the first build (`exact new --name --bundle-id`,
  [porting-and-release](porting-and-release.md)); `app.ts` takes the id from here.
- **`host.ios.launchImage`** `{ "src": "assets/splash.png", "width": 276 }`: the launch
  screen's PNG, `width` points wide, on `background_color`.
- **`host.ios.icon`** `"art/App.icon"`: an Icon Composer `.icon`; the web keeps `icons`.
- **`protocol_handlers`**: `euxy://p/abc` is `/p/euxy%3A%2F%2Fp%2Fabc` (whole at `%s`).
  A cold launch opens it in its route's tab: declare it (`p "/p/:d"`). Running, `go`
  pushes it on the selected tab; `open` uses its route's (`vocab navigate`).
- **Other keys:** `icons`, `data` (the `data-*` words a hatch reads), `tamber` (the design's
  file), `strings` (string tables), `typescript.sources` (outside directories `app.ts`
  imports), `commands` (the app's own `exact.mjs` verbs), `env` (settings read at build and
  inlined into the module on every host, so a secret does not belong there:
  [rest-backend](rest-backend.md)).
- **`host.ios.appClip`** `{}`: the app as an App Clip too (`id` defaults to `<app id>.Clip`;
  each `applinks:` domain is its `appclips:` one).
- **Not here:** grants (in `app.ts`), fonts and sounds (Contract).
- **A change to `app.json`, `assets/`, Swift, grants, `appId`, fonts or strings rebuilds** the
  native app (`bun exact.mjs ios`). An edit to `.contract` files, to `app.ts` (short of its
  grants, `appId` and canvas surfaces) or what it imports reaches a running build in seconds;
  with a Rust data crate, a file the build reads rebuilds ([ios-driving](ios-driving.md)).
