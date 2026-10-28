# Recipe: devices on the local network

A light, a printer or a NAS is found by DNS-SD with `@exact/local-network` and
reached with `fetch` (LLP 1133.036). `apps/lights` is the whole app.

1. **Declare** the purpose, each service type, and the reach:

   ```ts
   export const grants = 'device.local-network lanPurpose\nnet.dns-sd _wled._tcp\nnet.fetch local-network';
   ```

   `strings/en.json`: `{ "lanPurpose": "Lights finds and controls the WLED lights on your network." }`,
   the text of Apple's Local Network alert.

2. **Browse** where the list is shown; resolve, in data code, only the instance
   about to be used, and again each time (an address changes):

   ```ts
   const s = await resolveService({ name, type: '_wled._tcp' });
   const r = await fetch(`http://${s.host}:${s.port}/json/si`);
   ```

3. **Explain and recover** in the app's own words:

```text
use browseServices, localNetworkStatus, LocalServices, LocalNetworkStatus from "@exact/local-network"

component App
  resource network = localNetworkStatus() as shape LocalNetworkStatus else empty()
  resource found = browseServices(type="_wled._tcp") as shape LocalServices else empty()
  action openSettings
    openURL("app-settings:")
  view
    column
      if network.available and (network.state == "prompt" or network.state == "unknown")
        text "Lights finds the WLED lights on your network. Your device asks once."
      if network.state == "denied"
        text "Local Network access is off. Turn it on in Settings, or add a light by its address."
        button press=openSettings
          text "Open Settings"
      if not network.available
        text "This device can't look for lights. Add one by its address."
```

- `unknown` is `prompt`: Apple says nothing until a reach.
- **Always offer a typed address**: guest networks drop multicast, and the web,
  Linux and Windows do not browse. It is reached under the same line.
- Know a device by what it states (WLED's `mac`), not its instance name.
- A tool's or a wake's browse only joins one the app runs, and its local
  `fetch` fails `NotAllowedError` (`person-required`) until access is allowed.

**Under the agent** the network is the drive's (macOS, iOS, Android; the web
and Linux read `available` false):

```sh
bun exact.mjs agent macos --permission local-network=prompt \
  'prefer extension local-network.browseServices=[{"name":"Desk","type":"_wled._tcp","host":"127.0.0.1","port":4790}]' \
  'tap @1 allow' tree
```

A development build admits loopback under `net.fetch local-network`, so a
fixture on `127.0.0.1` stands in for the device (`apps/lights/fixture/server.ts`).
