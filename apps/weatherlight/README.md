# Weatherlight

A weather app in Exact2: one Contract view, a TypeScript provider using the host's
network bindings, and a separately loaded WebGPU sky. The sky follows the selected
hour's cloud cover, precipitation, wind, daylight, and city-local time. Forecast
exploration and Celsius/Fahrenheit conversion reuse the loaded data.

Run from the repository root:

```sh
node host/web/dev.mjs --app weatherlight
node host/apple/build.mjs weatherlight-apple --bundle --run --url http://127.0.0.1:8765/
node host/apple/build.mjs weatherlight-apple --ios --run --url http://127.0.0.1:8765/
```

Keep the dev server running for live edits. Its URL carries the matching plan,
bytecode, receipt and assets; a TypeScript edit does not rewrite the initial
`dist/app.plan`. Without `--url`, the native command opens the baked app.
`EXACT_WEB_DIST` can isolate the dev server's output from another app's build.

City search accepts a city or postal code; Enter submits. The presets offer a quick
trip between San Francisco, London, Tokyo, and Reykjavík. Choose an hour to preview
its sky and conditions; “Back to now” restores current weather. The pause button
freezes the sky. Refresh fetches again, and the app checks every ten minutes.

The first frame contains an honest empty forecast, baked without network access.
The first clock tick starts the request after paint. For an agent session, advance
`clock +1000`, then `clock settle` to finish the network request. A failed refresh
retains only that city's last good forecast, labeled stale; this cache lasts for
the session. Loading placeholders keep the hourly and daily sections the same
size, and a refresh keeps the current city’s data and scroll position. The page
uses the host's document scroller; only the hourly strip scrolls independently,
on its horizontal axis. No location
permission or API key is needed.

Weather and geocoding: [Open-Meteo](https://open-meteo.com/), CC BY 4.0. Forecasts
are model estimates, not a weather-alert service. The public endpoint is for
non-commercial use; commercial distribution needs the provider's subscription.
Times remain in the selected city's timezone. The landscape is illustrative.
The UI remains usable with its solid sky background if WebGPU is unavailable.

The app uses the shipped seams described by LLP 1009 (GPU module), LLP 1014
(canvas children), LLP 1016 (host requests), and LLP 1027 (deferred TypeScript).
`gpu/shaders/weather.wgsl` is reflected and validated at build. No GPU code is
linked into the app host, and no app JavaScript runs before first pixel.

Native data tests exercise the actual baked bytecode against controlled HTTP
responses; the GPU test draws and reads back actual frames, checking changing
weather, clock progression, and pause stability.
