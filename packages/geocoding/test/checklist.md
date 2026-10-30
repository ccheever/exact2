# @exact/geocoding: drives by hand

What the agent cannot drive: the platform's geocoder answering from Apple's
or Google's servers (LLP 1133.021 D6, D8). Weatherlight names the city it
follows with `reverseGeocode`. Use LLP 1133.020's test devices, and record
the date, the OS version, the device and what happened beside each step.

## iPhone (iOS 26 or later, and one on iOS 17 or 18)

`bun host/apple/build.mjs --ios --device --run weatherlight-apple`.

1. **Named.** Tap "Use my location" and allow location. Within a few
   seconds see the city you are in and its country in place of "Current
   location" (MapKit's request on 26; `CLGeocoder` before).
2. **No network.** Turn on Airplane Mode, tap "Use my location" again: the
   label stays "Current location" (the position is cached; the geocoder
   fails `NetworkError`).
3. **Language.** Set the phone's language to French and repeat 1: the
   country reads in French.

## Mac

`bun host/apple/build.mjs --run weatherlight-apple`: step 1 above.

## Android phone

`bun scripts/agent-android.mjs build weatherlight --apk`, installed. Step 1
above (the platform's `Geocoder`; on a phone without a geocoder backend
`Geocoder.isPresent()` is false and the label stays "Current location").

## Web, Linux, Windows

The web build: `bun host/web/dev.mjs --app weatherlight`. "Use my location"
answers "Current location" and never sends the position to a geocoding
service (D7).
