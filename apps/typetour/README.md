# Type Tour

A handful of phone-OS screens set entirely in two faces from the Expose type
program, to feel them in context on a real phone: **Exciting Lilt** (the
display face: Semibold and Bold, 17 px and up) for clocks, titles, marks, and
the few alerts that deserve attention, and **Exposure Sans Original 10** (the
text face: Regular, Medium, Semibold, Bold, and an Italic) for everything else.
Lilt's Bold sets titles and actions from 17 to 36 px, where it matches Sans
Bold's colour. Its Semibold sets the clock, the widgets' large numerals and the
48 px-and-up sizes.

Nothing here is a real app. The screens are authored in `app.contract`, the
taps move between them, and the data is the words on the screen. There is no
data crate to speak of (`data/` answers nothing) and no JavaScript.

- **Lock screen** — the clock in Lilt, four notifications. Tap the clock, the
  hint, or the Messages notifications to leave it.
- **Home screen** — two widgets, sixteen icons whose marks are Lilt, a dock.
  Messages, Settings, and Type Tour open; the rest stay put. Tap the time to
  lock the phone again.
- **Settings** — a Lilt large title over an Exposure Sans list, with working
  Airplane Mode and Bluetooth switches. *Display & Text Size* has a text-size
  chooser that scales Messages and the Settings list, and a specimen of both
  families side by side.
- **Messages** — six conversations, each openable, with a composer.

The home indicator at the bottom of every screen goes home.

The faces are copies of `fonts/exciting-lilt/ExcitingLilt-{Semibold,Bold}.ttf`
(v10, 2026-09-29) and `fonts/exposure-sans/original-10/*.ttf` (0.010) from the
Expose repository, MIT-licensed there; re-copy them into `assets/` when a new
build lands. Both families are declared in the Contract and bound to these
bytes (LLP 1019 D3), so an installed font of the same name is never consulted.

Run from the Exact2 root:

```sh
bun host/web/dev.mjs --app typetour                 # the dev loop; open the LAN URL on any phone
bun host/apple/build.mjs --device typetour-apple --run   # a connected, provisioned iPhone
bun host/apple/build.mjs --ios typetour-apple --run      # a simulator
bun host/apple/build.mjs typetour-apple --run            # macOS, in a phone-sized window
```

Drive it:

```sh
bun scripts/agent.mjs --app typetour web --url http://127.0.0.1:8765/ \
  "tap lock-unlock" "tap dock-messages" "tap conversation-nico" "screenshot out.png"
```
