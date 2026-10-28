# The local network by a person (LLP 1133.036 §10 drive 9, §14 stages 1 and 2)

What no tool here can do: show the platforms' Local Network alerts, find a
real device, and lose it at the wall. Use LLP 1133.020's test phones, and
write down the commit, the device and its OS version beside each result.

**What you need.** A WLED light: any ESP32 board flashed with WLED
(https://install.wled.me), joined to the same Wi-Fi as the phone, and WLED's
own app on the same phone as the reference. Lights is `apps/lights`.

**Your own network only.** Each step browses `_wled._tcp`, which finds every
WLED light on the network the device is on. Run it at home or on a test
network, never on a shared office or guest network.

## iPhone (iOS 14 or later): Lights

Build: `bun host/apple/build.mjs lights-apple --ios --device --run`.

1. **The alert.** On a fresh install Lights opens with "Lights finds the WLED
   lights on your network. Your device asks once." and iOS shows "“Lights”
   would like to find and connect to devices on your local network" with
   "Lights finds and controls the WLED lights on your network." Allow.
2. **Found.** The light is listed by its WLED name within a few seconds,
   with its switch and brightness slider as the light is.
3. **Switch and dim.** Turn it off and on, drag the slider: the light follows
   within a second. Change it in WLED's own app: tap Try again in Lights,
   and it reads the change.
4. **Lost.** Cut the light's power at the wall. Record how long until Lights
   shows it "Not on this network" (iOS keeps the record until its TTL ends
   or a goodbye arrives; minutes are expected). Power it back: it returns.
5. **Denied.** Settings → Privacy & Security → Local Network → Lights off.
   Return to Lights: "Local Network access is off…" and Open Settings, which
   opens Lights' page in Settings. Turn it back on: the light is listed again.
6. **Typed address.** With access off, type the light's IP address (WLED's
   own app shows it) and Add: record whether iOS refuses the reach (it should:
   the address is on the local network) and that Lights says "Not
   responding. Try again." rather than an error. With access on, the added
   light is controllable.
7. **The reference.** Repeat 1 to 4 in WLED's own app (delete it first for its
   alert). Lights' alert, list and loss timing should match it: note any
   difference as a tell.

## iPhone: what TN3179 warns of (D8; the probe is not built)

8. **The first fetch while the alert is up.** Delete Lights, install it, and
   before answering the alert, tap Add with the light's address typed. Record
   whether the fetch fails at once ("Not responding") while the alert is up,
   and whether Try again after Allow reaches the light.
9. **A tool while undecided.** On a fresh install, before answering the
   alert, run a tool that refreshes the lights (`bun scripts/agent.mjs ios
   --device tool …` once the device driver lands; else record as not driven).
   It must fail `NotAllowedError: person-required` and the light must not
   change.
10. **A VPN.** With a VPN profile that sets `includeAllNetworks` and
    `excludeLocalNetworks` false, record whether a typed address reaches the
    light and what Lights' status reads.
11. **The fact after Settings.** After allowing, turn access off in Settings
    with Lights in the background, return, and record the status line Lights
    shows before and after the next browse. Without the probe, the fact moves
    only when a browse or resolve reports.

## Mac (macOS 15 or later): Lights

Build: `bun host/apple/build.mjs lights-apple --run`.

12. macOS asks once per user account on the first browse; Allow. Steps 2 to 5
    as on the iPhone, with System Settings → Privacy & Security → Local
    Network. On macOS 14 nothing asks and the status reads `granted`.

## Apple TV

13. `bun host/apple/build.mjs lights-apple --tvos --run` on an Apple TV on the
    same network: no alert (tvOS has no local network privacy); the light is
    listed and switches with the remote.

## Android phone (API 34 or later; another on API 31 to 33 where at hand)

Build: `bun scripts/agent-android.mjs build lights --apk`, then `adb install -r`
the APK it prints (`host/android/README.md`, "Building an app").

14. No dialog (the host targets API 36, where `INTERNET` covers the local
    network). The light is listed; switch and dim as on the iPhone.
15. **Resolve.** On API 34 or later the address comes from
    `registerServiceInfoCallback`; change the light's IP (reserve another in
    the router, reboot the light): Lights follows without a relaunch. On API
    33 or earlier record that the one-shot resolve answers and a second light
    resolves after the first (resolves queue).
16. **Multicast lock.** On an Android 12 phone (before T extensions 7) the
    browse holds a multicast lock: `adb shell dumpsys wifi | grep -i
    multicast` lists `exact.local-network` while Lights is open and none after
    it is backgrounded. Elsewhere none is held.
17. **Background.** Background Lights for a minute and return: the list is
    answered afresh.

## Web (Chrome on a Mac)

18. Serve Lights' production build from a public origin (a deploy). The page
    says "This device can't look for lights" with the address field. Type the
    light's address: Chrome shows its Local Network Access prompt; Allow: the
    light is controllable. Firefox 153 or later asks likewise.

## Linux and Windows

19. Lights says "This device can't look for lights" (no browser is built on
    either) and a typed address works. On the NucBox record that Windows
    Firewall asks nothing for an outgoing reach.
