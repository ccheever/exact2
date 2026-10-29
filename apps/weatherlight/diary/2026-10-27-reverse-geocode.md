surfaces: web, macos, ios, android
agent: Claude Code, Claude Opus 5.5
task: name the city for the device's position

## Rough

- `use reverseGeocode, Place from "@exact/geocoding"` failed with
  `contract-use-shadows`, because the app already declares its own `Place` for
  search results. `use Place as GeoPlace` fixed it. The message named the fix.
  About 2 minutes.
- On the web the page still sent `reverseGeocode`, which the web refuses
  `NotSupportedError`: the home route is rendered at build, and the
  render read `geocodingStatus` by the Mac's answers, `available` true,
  which the page adopted. Fixed in exact2: the render host answers
  packages as the web does (`host/render/src/lib.rs`). About 40 minutes.

## Lean in

- `geocodingStatus().available` lets the app skip the request on the web and
  Linux, where there is no platform geocoder, so the label stays "Current
  location" there without special cases.

## Needed

- location: provided
- geocoding: provided (`@exact/geocoding`)

## Checkpoints

- editing app.contract: rough (see Rough)
