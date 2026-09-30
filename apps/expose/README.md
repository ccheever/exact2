# Expose

A phone operating system's face, in Exact2: a lock screen, a home screen, a
Messages app whose pinned thread is an assistant, a Reader with five articles,
and Settings — all set in the three faces from the Expose type program.

- **Exciting** (Black, v18) speaks loudest: the clock, the marks on the app
  tiles, the Reader masthead. One word at a time, never a sentence.
- **Exciting Lilt** (Semibold and Bold, v10) carries titles, names and actions,
  17 px and up.
- **Exposure Sans Original 10** (Regular, Medium, Semibold, Bold, Italic)
  carries everything you read.

The system chrome stays gray. One warm colour (`#ff6a2b`) means "this needs
you": an unread dot, the send button, the assistant's caret while it writes.

## What is real

- **The clock** is the device's, through `exactTime()` and a once-a-second
  tick; the strings come from `app.ts` (`clock`).
- **The weather** on the home screen and lock screen is Palo Alto's, from
  Open-Meteo, fetched after first paint and every ten minutes.
- **The assistant** (the "Expose" thread in Messages, also the dock's E) answers
  from OpenRouter. On the web's page runtime the reply streams in (the module
  reads the events from the response body); on native hosts it arrives as one
  completion, because a host keeps only the newest undelivered stream message
  (LLP 1016.000 D4) and a token stream cannot afford a gap. It sees the thread
  and, when you press *Ask Expose* in Reader, the article. Leave the thread
  while it is writing and the answer arrives as a banner; on the lock screen it
  becomes a notification.
- **Settings → Intelligence** picks the model and holds the key, kept under
  `secret.keep` on the device. Display & Text Size scales the reading text and
  switches the scheme.

Everything else is authored: the human threads, the notifications, the
articles, the tiles that say "isn't in this demo yet".

## The key

The assistant needs an OpenRouter key. Two ways to give it one:

1. Paste it in **Settings → Intelligence** on the device. It is kept on that
   device only.
2. Bake it into your own build: `EXPOSE_OPENROUTER_KEY=sk-or-… bun host/apple/build.mjs …`.
   Each crate's `build.rs` writes `apps/expose/local.ts` from the variable
   (gitignored; an empty one is written when the variable is unset so a clean
   checkout builds). The dev loop reads the same file, so write it once by
   hand or run any native build first.

A key from Settings wins over a baked one. *Forget the saved key* in Settings
falls back to the baked one.

## Run

From the Exact2 root:

```sh
bun host/web/dev.mjs --app expose --lan                          # the dev loop; open the LAN URL on any phone
bun host/apple/build.mjs --device expose-apple --run --phone <udid>   # a connected, provisioned iPhone
bun host/apple/build.mjs --ios expose-apple --run                 # a simulator
bun host/apple/build.mjs expose-apple --run                       # macOS, a phone-sized window
```

Drive it: `bun scripts/agent.mjs --app expose web --url <url> "tap lock-unlock"
"tap app-messages" "tap thread-expose" "type composer hello" "tap send"
"clock settle" "screenshot out.png"`.

## Layout

- `app.contract` — the root component and the five screens.
- `parts.contract` — shapes, named styles, keyframes, and the leaf components
  (status bar, tiles, notices, settings rows, bubbles, the home bar).
- `app.ts` — the sources: clock strings, inbox and threads, the assistant,
  the articles, the forecast, the Intelligence settings.
- `assets/` — the fonts, copied from the Expose repository
  (`fonts/exciting/handoff/Exciting-Black.ttf`, `fonts/exciting-lilt/`,
  `fonts/exposure-sans/original-10/`); re-copy when a new build lands. Both
  families are declared in the Contract and bound to these bytes (LLP 1019
  D3), so an installed font of the same name is never consulted.
