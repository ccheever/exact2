# Harmony

A team chat in the Discord and Slack shape, written once in Exact and
shipped as an iPhone app, a Mac app and a web app, with Snapback4 as the
backend.

What it does: channels grouped by category with unread counts, direct
messages, message rows grouped by author, replies, edits, deletes, reactions
with counts, custom emoji (a one-colour shape anyone can add), pinned
messages, a members list, a typing indicator, search across every room, and
six accounts to switch between under You. There is no sign-in: the six are
Snapback development personas, each with its own device partition.

## Run it

```sh
bun install                              # snapback4@0.4.33, in apps/harmony
bunx snapback4 dev --memory --port 4400  # the backend, in apps/harmony, empty
cd app && bun exact.mjs seed             # demo data: six people, eight channels, three days of chat
bun exact.mjs web                        # http://127.0.0.1:<port>, rebuilt on save
bun exact.mjs mac --run                  # the Mac app
bun exact.mjs ios --run                  # an iPhone simulator
bun ../../../scripts/memory-backend-test.mjs apps/harmony/app   # the tests (the app named from exact2's root)
```

The tests run against a server of their own: `scripts/memory-backend-test.mjs`
starts `snapback4 dev --memory` on a free port, builds the web app against it
(`EXACT_BACKEND_ORIGIN`) and stops it after. Each test empties the server it
reaches (`fresh`), so never run `bun exact.mjs test web` against a server whose
data matters. The async lane runs them this way.

`bun exact.mjs seed` loads `seed.json` into a dev server nothing was written to (below), which
`seed.mjs` writes with times relative to now. The tests load the same file
before each test (`fresh "../seed.json"`).

`seed` seeds unasked only a server nothing was ever written to (its commit
head is 0), erasing nothing. Any other may hold what people wrote, in memory or on disk (on the
machine that serves the demo, 4400's does), and seeding erases everyone's
messages, channels and profiles there, so it is refused, nothing erased, unless
`--force` (or `--reset`) says to empty it first: pass it only for a server of your own whose
data you mean to lose, such as one you seeded before.

The layout is the one `exact new --snapback` makes (LLP 1086): the backend in
this directory, the app in `app/` below it, so the app's crates are
`app-web`, `app-apple` and `app-linux` in the root workspace.
`snapback/generated/` is committed so the workspace builds on a clean
checkout; `bun exact.mjs snapback --rebuild` in `app/` rewrites it after a
change to `schema.q` or the Snapback pins.

Keep every read in the one `snapshot` source and every write admit-only
(`db.write`): natively the Snapback device serves one answer at a time
(`app/app.ts` says why).

## Where things are

| | |
|---|---|
| `snapback/schema.q` | the backend (`snapback/generated/` is what `snapback4 check` writes from it): rooms, members, messages, reactions, pins, typing, read marks, profiles, custom emoji; the queries and mutations |
| `app/app.contract` | the view: three tabs (Channels, Messages, You), a room screen, the sheets, the menus, the emoji picker |
| `app/app.ts` | the data module: one `snapshot` read for everything the view shows, and the writes, through `Snapback.switcher` (one partition per persona) |
| `app/app.tamber` | the brand: six avatar hues, the mention wash, the two icons the platforms lack |
| `app/app.test.contract` | six end-to-end tests on the web |
| `app/apple`, `app/linux` | the native crates: each links `exact_snapback4::Module` as the data module's native device |

## What the platforms draw for free

The tab bar is a sidebar on the Mac and in a wide browser window; a channel
row pushes its room beside the list; create sheets are sheets; the message
menu is a long press on iOS and a right-click on the Mac; dark mode, Dynamic
Type and the accent colour are the person's. The source says what things are
and leaves the rest to the platform.
