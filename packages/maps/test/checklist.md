# @exact/maps: drives by hand

What the agent cannot judge: whether the map looks and feels like the
platform's own, the real tiles (the agent draws plain ones, LLP 1133.021
§12), and the Android map, which needs a Maps SDK key (§15 question 7).
Use LLP 1133.020's test devices. Record the date, the OS version, the
device and what happened beside each step when you run it.

## Web: Chrome, Safari and Firefox

Build and serve Map Demo: `bun host/web/dev.mjs --app map-demo`, then open
the URL it prints.

1. **Tiles.** See OpenStreetMap's map of San Francisco's north-east corner
   at street level, four red pins with their names below, and
   "© OpenStreetMap contributors" at the bottom right.
2. **Select on the map.** Click the Ferry Building pin. See it grow and
   darken, "Selected: ferry" at the top, and "Pins selected on the map: 1".
3. **Select by button.** Click Coit Tower below the map. See the Coit Tower
   pin selected and the count unchanged.
4. **Gestures.** Drag the map; scroll the wheel over it (or pinch on a
   trackpad); the map pans and zooms about the pointer. After each, the
   "Zoom:" line changes to the zoom the map came to rest at.
5. **Keyboard.** Tab to the map; the arrow keys pan it, `+` and `-` zoom.
6. **Screen reader.** With VoiceOver (Safari) the map is announced as "Map of
   stores…" and each pin by its title.

## iPhone (iOS 17 or later) and iPad

`bun host/apple/build.mjs --ios --device --run map-demo-apple`.

1. **The platform's map.** See MapKit's map with four red balloon markers,
   titles below them, Apple's legal label at the bottom left. In Dark Mode
   the map is dark.
2. **Select.** Tap the Ferry Building balloon: it enlarges, "Selected:
   ferry", the count 1. Tap the map away from the markers: "No store
   selected".
3. **Gestures.** Pan, pinch, rotate with two fingers (the compass appears),
   tilt with a two-finger vertical drag. The "Zoom:" line follows each
   pinch once the map rests.
4. **VoiceOver.** Each marker is an element named by its title.
5. **Time to the map.** From launch, note how long the map's box stays
   empty before the map shows (LLP 1133.021 D21: the package loads after
   first pixel). Record it.

## Mac (macOS 14 or later)

`bun host/apple/build.mjs --run map-demo-apple`. The same as the iPhone,
with a trackpad: pinch, rotate, scroll to pan, the zoom controls at the
bottom right; click a marker to select it.

## Apple TV (tvOS 17 or later)

`bun host/apple/build.mjs --tvos --device --run map-demo-apple`. The zoom
buttons show (no pointer); press Zoom in: the map zooms one step and the
"Zoom:" line reads one more. Swipe on the remote to pan. The map neither
rotates nor tilts.

## Android phone with Google Play services

Needs a Maps SDK key restricted to `com.exact.mapdemo` and the debug
signing certificate's SHA-1 (LLP 1133.020 §3.2 item 6), given privately.
Never commit it.

`EXACT_GOOGLE_MAPS_API_KEY=<key> bun scripts/agent-android.mjs build map-demo --apk`,
then install the APK. See `packages/maps/test/android.mjs` for what the
build checks.

1. **The map.** Google's map with four red markers. Without the key (a
   development build), the SDK's blank grey map, and the build warned.
2. **Select.** Tap a marker: "Selected: …", the count 1. Tap a button
   below: that marker selected, the count unchanged.
3. **Gestures.** Pan, pinch, rotate, tilt. The "Zoom:" line follows once the
   camera is idle. If the map does not pan, hosted views do not yet take
   touches (LLP 1133.021 §9 item 4): record it.
4. **Lifecycle.** Leave the app and come back; rotate the phone. The map
   keeps its camera and markers.
5. **A phone without Play services** (if one is at hand): the SDK's own
   message in the map's box.
