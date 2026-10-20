# Recipe: app.json, assets and modules

`app.json` names the app and what it ships; its `$schema` gives an editor every key with its
description. Files the app carries are folders beside it, not keys.

```json
{
  "$schema": "../exact2/scripts/app.schema.json",
  "app": { "id": "com.yourcompany.euxy", "name": "Euxy" },
  "name": "Euxy",
  "modules": ["waveform-view"],
  "audio_session": "playback",
  "icons": [{ "src": "assets/icon-1024.png", "sizes": "1024x1024", "type": "image/png" }],
  "typescript": { "sources": { "core": "../shared/core" } },
  "commands": { "seed": ["bun", "tools/seed.mjs"] }
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
- **`modules/apple/*.swift`** (and `*.xcframework`) is the app's native module, built whenever
  the folder has one; `modules/web/index.js` answers on the web
  ([native-module](native-module.md)). `modules` lists only the hyphenated tags of native
  views ([native-views](native-views.md)): a tag the list lacks is `bake-unknown-module`.
- **`audio_session`**: `"playback"` for an app whose sound is the point (the silent switch
  does not mute it); the default `ambient` mixes with other apps' audio.
- **`app.id` and `app.name`** are set before the first build (`exact new --name --bundle-id`
  writes them, [porting-and-release](porting-and-release.md)); `app.ts`'s `appId` matches.
- **`host.ios.launchImage`** `{ "src": "assets/splash.png", "width": 276 }`: a PNG the launch
  screen centres on `background_color`, `width` points wide.
- **`host.ios.icon`** `"assets/App.icon"`: an Icon Composer `.icon`, in place of `icons`.
- **`protocol_handlers`** `[{ "protocol": "euxy", "url": "/p/%s" }]`: a custom URL scheme.
  `euxy://p/abc` reaches the navigation root whole at `%s`: `/p/euxy%3A%2F%2Fp%2Fabc`.
- **Other keys:** `icons`, `data` (the `data-*`
  words a hatch reads), `tamber` (the design's file, else `app.tamber`), `strings` (string
  tables), `typescript.sources` (directories outside the app that `app.ts` imports),
  `commands` (the app's own `bun exact.mjs <verb>`).
- **Not here:** grants are `app.ts`'s `export const grants`; fonts and sounds are declared in
  Contract.
- **A change to `app.json`, `assets/`, Swift, grants, `appId`, fonts or strings rebuilds** the
  native app (`bun exact.mjs ios`). In an app whose data is `app.ts`, an edit to `.contract`
  files, to `app.ts` itself (short of its grants, `appId` and canvas surfaces) or to what it
  imports reaches a running build in seconds; with a Rust data crate, a file the build reads
  rebuilds ([ios-driving](ios-driving.md)).
