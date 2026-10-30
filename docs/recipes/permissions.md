# Recipe: a capability that asks permission

A native module that needs the person's permission (location here) is asked
for in the app's own UI, read as a fact, and recovered from in the app's own
UI. exact2 never answers the platform's prompt (LLP 1133 D7).

1. **Declare** one grant line, and its purpose in the strings table. The build
   derives every platform's manifest from it.

   ```ts
   export const grants = 'device.geolocation-approximate locationPurpose';
   ```

   `strings/en.json`: `{ "locationPurpose": "Shows the weather where you are." }`.
   `device.geolocation` is precise; write one line, not both.

2. **Read** the state, which is a fact: `granted`, `prompt`, `denied` or
   `unknown`, with `available` and `accuracy`.

3. **Ask** from a person's action. The request itself shows the platform's
   prompt while the state is `prompt`.

4. **Recover:** while `denied`, say so and offer Settings; everything that
   does not need it keeps working.

```text
use getCurrentPosition, geolocationStatus, GeolocationPosition, GeolocationStatus from "@exact/geolocation"

component App
  resource location = geolocationStatus() as shape GeolocationStatus else empty()
  mutation located as shape GeolocationPosition then arrived
  action useLocation
    send located = getCurrentPosition(maximumAge=600000, apple.desiredAccuracy="threeKilometers")
  action arrived
    match located
      case some(p)
        lat = p.coords.latitude
      case none
        lat = lat
  action openSettings
    openURL("app-settings:")
  view
    column
      if location.available && location.state != "denied"
        button press=useLocation testId="use-location"
          text "Use my location"
      if location.state == "denied"
        text "Location is off. Turn it on in Settings, or choose a city." testId="location-denied"
        button press=openSettings testId="open-settings"
          text "Open Settings"
```

A module's options are keyword arguments; a platform's own words are dotted
(`apple.desiredAccuracy`, `android.quality`), and an omitted one stays the
platform's default. Data code reads the same names:

```ts
import { watchPosition } from '@exact/geolocation';
const here = await watchPosition({ maximumAge: 600000, apple: { distanceFilter: 5000 } });
```

A resource that read it is asked again when it changes; an error rejects with
`{ code, message }`.

**Under the agent** nothing reaches the platform; the drive answers:

- `--permission geolocation=granted` at launch, or `prefer
  permission.geolocation denied` mid-drive;
- a request while `prompt` waits on a `permission` ticket in `state.pending`:
  `tap @t allow`, `deny` or `cancel`;
- a position with none preferred waits on a `position` ticket: `type @t
  51.5074,-0.1278` (latitude, longitude, accuracy);
- `prefer extension geolocation.watchPosition={"coords":{"latitude":48.8566,"longitude":2.3522}}`
  sets the position, and readers are asked again;
- `state.extensions` lists readers, preferred values and tickets.

`apps/weatherlight` is the whole example.
