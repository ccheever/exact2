# Height and pressure by a person (LLP 1133.026 §15, stages 1 and 3)

What no tool can do here: carry a barometer up a hill, and leave the app
suspended while it moves. `linux.mjs` drives the declared substitute; a
simulator and an emulator report no barometer (the emulator's virtual
sensors can set pressure, which stage 3's drive uses). Use LLP 1133.020's
test phones. Write down the commit, the phone and its OS version beside
each result.

The conformance fixture is the pedometer's (`packages/pedometer/test`):
`climb` shows the relative altitude, its `since` and `interrupted`; `air`
the pressure and the batch's sample count. Fieldnotes' walk line shows the
climb as "↑ N m". The pedometer's checklist says how to install both.

## iPhone with a barometer (iPhone 6 or later)

1. **One decision.** On a fresh install, the fixture's Ask shows one
   Motion & Fitness alert for both modules (the pedometer's checklist,
   step 1). After it, `height-status` reads `true granted`.
2. **Pressure.** `air` reads the local pressure in hectopascals (about
   1013 at sea level; compare a weather report) and its sample count grows
   to 16.
3. **A climb.** Climb two flights of stairs: `climb` rises by about 6 m
   within a few seconds of the top. Its `since` stays the moment of the
   first reading.
4. **The reference across a suspension.** Note `climb`, lock the phone for
   five minutes without moving, unlock: record whether the height comes
   back within a metre of the noted one, and whether the app's log shows
   "a new reference after the return" (D13: whether Core Motion keeps the
   reference across a suspension). The next answer reads `interrupted`
   `true`.
5. **Denied.** Turn Motion & Fitness off for the app in Settings:
   `height-status` reads `true denied` and `climb-error` `NotAllowedError`.

## Android phone with a pressure sensor

1. **No prompt.** `height-status` reads `true granted` before anything is
   asked, and Ask shows only the pedometer's Physical activity dialog.
2. **Pressure and a climb.** As the iPhone's 2 and 3. Record how far the
   height drifts with the phone still on a table for an hour (weather
   moves pressure; D13 declares it).
3. **Across the background.** Note `climb`, background the app for five
   minutes without moving, return: the height is within a metre of the
   noted one (the zero is kept by construction), and `interrupted` reads
   `true`.
