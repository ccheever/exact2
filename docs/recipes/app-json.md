# Recipe: app.json, assets and modules

`app.json` names the app and what it ships; its `$schema` gives an editor every key. Files
the app carries are folders beside it.

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
    column padding=16 gap=8
      image "assets/logo.png" width=96 object-fit="contain" aria-label="Euxy"
      button press=tick testId="tick"
        text "Tick"
```

- **`assets/`** is bundled whole on every host. Contract names a file by its path from the app,
  with no leading slash: `image "assets/logo.png"`, a `sound` and `playSound`, a `font` block's
  `400 = "assets/Inter-Regular.ttf"` (then `font-family="Inter"`).
- **`modules/apple/*.swift`** (or `*.xcframework`) is the app's native module;
  `modules/web/index.js` answers on the web ([native-module](native-module.md)). `modules`
  lists only the hyphenated tags of native views ([native-views](native-views.md)): a tag the
  list lacks is `bake-unknown-module`.
- **`audio_session`**: `"playback"` where sound is the point (the silent switch does not
  mute it); the default `ambient` mixes with other audio.
- **`app.id` and `app.name`** are set before the first build (`exact new --name --bundle-id`
  writes them, [porting-and-release](porting-and-release.md)); `app.ts`'s `appId` matches.
- **`host.ios.launchImage`** `{ "src": "assets/splash.png", "width": 276 }`: the launch
  screen's PNG, `width` points wide, on `background_color`.
- **`host.ios.icon`** `"art/App.icon"`: an Icon Composer `.icon`; the web keeps `icons`.
- **`protocol_handlers`**: `euxy://p/abc` is `/p/euxy%3A%2F%2Fp%2Fabc` (whole at `%s`).
  A cold launch opens it in its route's tab: declare it (`p "/p/:d"`). Running, `go` can
  push it on the selected tab; `open` uses its route's (`vocab navigate`).
- **Other keys:** `icons`, `data` (the `data-*` words a hatch reads), `tamber` (the design's
  file, else `app.tamber`), `strings` (string tables), `typescript.sources` (directories
  `app.ts` imports from outside the app), `commands` (the app's own `exact.mjs` verbs).
- **`host.ios.appClip`** `{}`: the whole app as an App Clip too (`id` defaults to
  `<app id>.Clip`; each `applinks:` domain is its `appclips:` one).
- **Not here:** grants (`app.ts`'s `export const grants`), fonts and sounds (Contract).
- **A change to `app.json`, `assets/`, Swift, grants, `appId`, fonts or strings rebuilds** the
  native app (`bun exact.mjs ios`). An edit to `.contract` files, to `app.ts` (short of its
  grants, `appId` and canvas surfaces) or what it imports reaches a running build in seconds;
  with a Rust data crate, a file the build reads rebuilds ([ios-driving](ios-driving.md)).
