# Steps by a person (LLP 1133.026 §15, stages 1 and 3)

What no tool can do here: walk with a phone, answer the Motion & Fitness
prompt and the Physical activity prompt, and leave the app in a pocket.
The suites drive the rest under the agent: `linux.mjs` on the Linux host,
`apps/fieldnotes/walk.drives.mjs` on an iOS simulator, macOS and the web.
A simulator and an emulator have no step counter, so every step below is a
real phone's. Use LLP 1133.020's test phones, never a personal one. Write
down the commit, the phone and its OS version beside each result.

Two apps carry the module:

- **Fieldnotes** (`apps/fieldnotes`): Start walk and End walk in the
  header, and the walk's line under it.
  `bun host/apple/build.mjs --device fieldnotes-apple --run` on an iPhone.
- **The conformance fixture** (`test/fixture`), which shows each answer by
  itself. Make one with
  `bun -e "import('./packages/pedometer/test/fixture.mjs').then(m => console.log(m.makeFixture().app))"`,
  then `EXACT_APP_DIR=<that path> bun host/apple/build.mjs --device stepfixture-apple --run`,
  or `bun exact.mjs android` in that directory for an Android phone.

## iPhone, iOS 17 or later

1. **One prompt for both sensors.** Install Fieldnotes fresh. Start walk
   shows in the header. Tap it: one Motion & Fitness alert shows,
   carrying the walk's purpose ("Fieldnotes counts your steps …"). Allow:
   the line "Walking · 0 min" shows, with steps and height as they come.
   Record that only one alert showed, and that the fixture's
   `steps-status` and `height-status` then both read `true granted`
   (the two statuses read one decision, D15).
2. **A walk.** Walk for ten minutes with the phone in hand, up a flight of
   stairs or a hill if one is near. The steps rise every few seconds; the
   height reads "↑ N m" and rises on the stairs; "· N.N km" shows where
   the phone measures distance.
3. **A pocketed walk.** With a walk running, lock the phone and walk for
   five minutes. Unlock: within a second the steps include the pocketed
   walk (Core Motion counts while the app is suspended).
4. **Through a relaunch.** Swipe Fieldnotes away mid-walk and open it
   again: the line comes back with the same start and duration, and the
   steps since the start.
5. **Denied.** Settings › Privacy & Security › Motion & Fitness ›
   Fieldnotes off, then return: Start walk shows the line "Fieldnotes
   can't count your steps …" and asks nothing. Turn it on again and
   return: Start walk starts a walk.
6. **End walk.** End walk shows the summary in the header. Close and open
   Fieldnotes: no summary, and no note mentions the walk.
7. **A start older than a week.** In the fixture, the steps' `since` is
   the first ask's moment. Record what `count` reads after the phone's
   date is moved eight days ahead in Settings (D12: whether Core Motion
   moves `startDate`); move it back after.

## Android phone, API 29 or later

1. **The prompt.** Install the fixture fresh. Tap Ask: Android's Physical
   activity dialog shows. Allow: `steps-status` reads `true granted`,
   `height-status` `true granted` (the altimeter asks nothing), and
   `count` starts at 0.
2. **The Recording path.** On a phone with current Google Play services,
   walk for five minutes with the app open: `count` rises step by step.
   Background the app and walk another five minutes; return: the count
   includes them, and `since` is unchanged.
3. **The sensor path.** On a phone without Google Play services (or with
   it disabled), repeat 2: in the foreground the count rises; after the
   background walk `since` moves to the return and the count restarts
   from 0 (D12: no steps it could not have seen).
4. **The hidden poll.** With the app's activity in the background for ten
   minutes, return: the count shows at once, never an error line
   (`count-error` empty).
5. **Denied.** Settings › Apps › the fixture › Permissions › Physical
   activity › Don't allow: `steps-status` reads `true denied` and
   `count-error` `NotAllowedError`.
