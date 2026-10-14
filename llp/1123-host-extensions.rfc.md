# LLP 1123: Host extensions

**Type:** RFC (a decision brief: each decision carries a proposed answer; nothing here is built in exact2 until Charlie has read it)
**Status:** Draft r1, 2026-10-14 (drafted 2026-10-13 as 1121, renumbered when LLP 1121, Tamber, landed). Not reviewed. Proposed from outside the working set; it needs Charlie's ruling and, if accepted, a take or a waiver under `rules/DEFERRED.md` (§7, question 1). Not in `llp/current/`, which holds 15 of 15; adding it means archiving one, which is Charlie's choice.
**Systems:** the app manifest (`scripts/app.schema.json`, `contract/cli/src/manifest.rs`), the bake's checks (`contract/cli`), the plan's record of what an app needs (`plan/`), the runner's native slot and topics (`runner/src/runner/source.rs`, `commit.rs`), the agent's `prefer` and `state` (`runner/src/agent.rs`, each host's agent), each host's module build (`host/apple/build.mjs`, `host/web/native-glue.js`, `host/linux`, `host/windows`, `host/android`)
**Author:** Douglas Lowder, with Claude (Opus 5.5)
**Date:** 2026-10-14
**Implementer:** none yet (a spike on today's seams is §6; a spec follows a ruling)
**Related:** LLP 1024 (native modules; "the roster is packaging, not language"); LLP 1067.000 (one native module per app: `call`, `later`, topics, agent mode); LLP 1016.002 (push invalidates, the answer stays a pull); LLP 1027 D8 (sources routed by name); LLP 1069.000 D4 (battery and thermal state: "a native module"); LLP 1069.007 (capabilities in the agent's hands; D7, the Substitute paragraph); LLP 1075.003 §3.3 and §9.1 (declared `data-*` words, `querySelectorAll`); LLP 1075.003.000.001 (hatches on every platform); LLP 1077 D14 (`haptic`, a host command); LLP 1121 D13 (Tamber's `mode`, the fourth `prefer` group)

## Summary

A **host extension** is a named, versioned file that declares typed data
sources, sends and `data-*` words, with an implementation for each platform
that supports it. An app lists the extensions it uses, by name and version,
in `app.json`. The bake checks the app against the files, the plan records
the list, and the build puts each implementation into the app's one native
module. At run time an extension's sources and sends reach that module
through the native slot, as `native.later` and `native.call` reach it
today; a live source announces a topic. Under the agent, each source is a
`prefer` name. There is no new agent operation, no new Contract syntax and
no new host command.

A spike on today's exact2 (§6) runs the design on the iOS simulator, on an
iPhone, where the app shows the phone's real battery level and state, and on
the web, where a second extension reads the device's orientation.

An extension is the app's native module (LLP 1067.000), cut into pieces
that are written once and shared: by the apps that need the same device
value, and by a host that has that value for every app it runs.

## 1. Motivation

An app that needs a device value with no web name (battery, thermal state,
an accelerometer, a badge's profile record) writes its own native module
today, once per platform, and its own data source that calls it. Two apps
that need the battery write it twice. A host that has an accelerometer
cannot offer it to every app it runs, and nothing tells a host, before it
runs an app, that the app needs a value the host does not have.

The need comes from an out-of-tree port of Contract apps to an ESP32-S3
board (the M5Stack Stopwatch: a display, a touch panel, an accelerometer, a
battery and a vibration motor). Its apps needed:

- `attitude()`: pitch and roll from the accelerometer, which change by
  themselves;
- `badgeProfile()`, `badgeSettings()`, `badgeBattery()`: records the badge
  firmware keeps in flash, the last of which changes by itself;
- `setBrightness(level)`: a send that changes the panel;
- `data-snap`, `data-snap-align`: words a native scroller reads.

None of these has a web name, and each would be a new reserved source, host
command or per-app module under today's rules. That port declared each in a
file and checked the app against it at build; the port itself is not part of
this proposal. This RFC proposes the same declaration on exact2's own hosts,
where the plan, runner and kernel run on the device, on the seams they
already have.

## 2. What exists

| Need | exact2 today | Gap |
|---|---|---|
| A device value with no web name | The app's module answers `native.call` / `native.later`; the app's data source (Rust or TypeScript) calls it (LLP 1067.000 D1, D5) | Per app, per platform; untyped JSON |
| A value that changes by itself | `native.watch(topic)`; the module announces the topic; the runner asks again (LLP 1016.002) | None |
| A fact every host gives | A reserved source (`exactPage`, `exactViewport`), checked field by field at bake (LLP 1069.000 D2) | Not for values only some hosts have (D4) |
| An action on the device | `HOST_COMMANDS`, a closed list (`contract/syntax/src/lib.rs:87`); a send to a mutation, answered by the data source | — |
| A node's own words | Declared `data-*` words (`app.json` `data`), read by hatches and `querySelectorAll` (LLP 1075.003 §3.3, §9.1) | — |
| The agent | Ten operations; a fact is a `prefer` name; a request is a held ticket (LLP 1069.007 D1–D4) | The module substitute is owed (LLP 1069.007 D5) |
| What an app needs from a host | `modules`, `hatches`, `data` in `app.json`; hatch words checked per platform at delivery (LLP 1075.003.000.001 Q6) | No list of device values, so no check |

## 3. Design

### D1 — An extension is a file and its implementations; packaging, not language

An extension is a directory:

```
extensions/battery/
  extension.json        the declaration
  apple/*.swift         an ExactExtension subclass (D6)
  web/index.js          an object with call/later/connect (D6)
  linux/*.rs            a type implementing exact_linux::Extension (D6)
```

`extension.json`:

```json
{
  "extension": "battery",
  "version": 1,
  "summary": "The device's battery: its charge from 0 to 1 (-1 when unknown) and its state.",
  "sources": [
    { "name": "battery", "live": true,
      "result": { "level": "number", "state": "string" },
      "default": { "level": -1, "state": "unknown" } }
  ],
  "sends": [],
  "data": []
}
```

- **Sources** have parameters and result fields, each `number`, `string`,
  `bool`, or an option of one; `live` says the answer changes by itself;
  `default` is the answer where no implementation answers (D7, D9), and
  each field's type default (0, `""`, `false`, none) where it is absent. A
  type default can mislead: a battery level of 0 is empty, not unknown.
- **Sends** have parameters and answer as a mutation does.
- **`data`** lists `data-*` words the extension reads from nodes (D8).
- Contract does not change. An app reads `battery()` as it reads any source
  (`resource b = battery() as shape Battery`) and sends as it sends to any
  mutation. The name space is the app's sources, as today.

`version` is an integer. A change that removes or retypes a field, a
parameter or a word is a new version; adding a source, a send or a word is
not. An implementation says which versions it answers.

### D2 — An app lists its extensions in `app.json`

```json
"extensions": { "battery": 1, "attitude": 1 }
```

By name and version, never by path. The build finds a name in the app's own
`extensions/` directory, then in exact2's `extensions/` (question 2). The app
does not list the extension's sources or words again.

### D3 — The bake checks the app against the files

For each source and send the plan uses that the app's own data source does
not answer, the bake finds the one listed extension that declares it, and
refuses by name:

- `bake-unknown-extension`: a listed name with no file;
- `bake-extension-version`: a version the file does not declare;
- `bake-extension-source`: a source or send no listed extension declares,
  where the app has no data source of its own that answers it;
- `bake-extension-field`: a field the app reads that the declaration lacks,
  or reads as another type; a send with another parameter count;
- `bake-extension-twice`: two listed extensions, or an extension and the
  app's own data, that declare one name.

This is the check `bake-page-field` makes for `exactPage` (LLP 1069.000 D2),
applied to a declared source.

### D4 — The plan records what the app needs

The plan carries a table: for each listed extension its name, version and
the SHA-256 of its `extension.json`, and the sources, sends and words the app
uses from it. A host reads it at load; the agent reads it in `state` (D7);
delivery reads it to say which platforms an app can run on (D9).

### D5 — An extension's sources and sends reach the module through the native slot

The runner routes a source or send that a listed extension declares to the
native slot, as LLP 1027 D8 routes a source by name to its language:

- A source is a `later` request to `exact-native:` with the body
  `{"extension": "battery", "source": "battery", "args": [...]}`; a source
  the file marks `sync` is a `call`.
- A live source's implementation announces the topic `battery.battery`
  (extension, then source). The runner asks again; the answer stays a pull
  (LLP 1016.002 D1, D4).
- A send is a `later` with `"send"` in place of `"source"`; its answer
  settles the mutation as a data source's answer does.
- The reply's JSON is checked against the declaration in development builds
  and refused by name when it does not match.

So the app's data source is not written for an extension's values, and a
Rust or TypeScript source still answers the names it owns.

### D6 — One module per app stays; the build composes it

The build puts each listed extension's implementation for the platform into
the app's one module artifact, beside the app's own module code. The
artifact, its ABI and its lifetime are LLP 1024 D3–D5 and LLP 1067.000
D2–D8 unchanged: loaded after first pixel, one instance per session, every
entry on the main thread, nothing fetched over the network, never closed.

| Host | Implementation | Composed into |
|---|---|---|
| macOS, iOS, tvOS | `apple/*.swift`, a subclass of `ExactExtension` with `call`, `later`, `destroy` and a context with `changed`, `agent`, `now` | `libexact_modules.dylib`; the module dispatches on `"extension"` |
| Web (JS target and wasm) | `web/index.js` exporting `later` and `connect` (the web has no synchronous `call`, LLP 1067.000 D9) | the page module that `native-glue.js` loads |
| Linux, Windows | `linux/*.rs`, a type implementing an `Extension` trait | the statically linked module, as hatches are (LLP 1075.003.000.001 D9) |
| Android | `android/*.kt` (provisional, as hatches are) | the app's Kotlin module |
| Terminal | none in v1 | — |

An app may implement an extension in its own module; a host may
ship one for every app it runs. A host that ships one takes the place of the
app's, never both (question 4).

### D7 — The agent: `prefer` and `state`, no new operation

**Substitute.**

1. **Shape:** each source is a fact (LLP 1069.007 D1); each send is a
   command whose answer the module gives.
2. **Fact:** a fifth `prefer` group, `extension`, beside `media`, `page`,
   `fold` and `mode` (LLP 1121 D13): `prefer extension
   battery.battery={"level":0.2,"state":"unplugged"}` sets the answer of the
   extension's source, and the wire carries it as
   `{"op":"prefer","extension":{"battery.battery":{…}}}`. While it is set the
   runner answers the source itself and does not call the module, on every
   host. `prefer extension battery.battery=clear` returns it to the module. A
   launch fixture (`--prefer extension battery.battery=…`) sets it before the
   first answer. With no `prefer`, the module's agent-mode
   substitute answers (LLP 1067.000 D6), and an extension with none answers
   its declared `default` (D1).
3. **Send:** answered by the module's agent-mode substitute, which journals
   it; no ticket is held. A send that opens system UI is a request under LLP
   1069.007 D3: the runner holds it as a ticket that the agent answers with
   `tap @t <choice>`, and the extension's file names the capability and its
   choices. A permission prompt is one: the §6 spike's `requestAttitude()`
   asks for motion access in iOS Safari and Chrome. Such an extension needs
   its own capability LLP, with the D7 paragraph that LLP 1069.007 asks for.
4. **Disclosure:** `state.extensions` lists each extension's name, version,
   SHA-256, whether this host implements it, and each source's preferred
   value. A field the file marks `secret` shows its presence only.
5. **Lifetime:** a preferred value lasts until cleared or the session ends.
6. **Web carrier:** none; the module substitutes.

This closes LLP 1069.007 D5 for extension sources.

### D8 — Words

An extension's `data` words join the app's declared `data` words, with
LLP 1075.003 §3.3's rules unchanged: a word names the app's meaning, not a
platform property, and is not an animation channel. A word declared by two
listed extensions, or by an extension and the app, is
`bake-extension-twice`. Hatches and `querySelectorAll` read the words as
they read any declared word; the typed keys the Apple build writes
(`ExactDataKey`) include them.

### D9 — A host without the implementation

The build for a platform refuses a listed extension with no implementation
for it (`bake-extension-platform`), unless the app marks it optional:

```json
"extensions": { "battery": { "version": 1, "optional": true } }
```

An optional extension a host lacks answers each source with its declared
`default` (D1) and refuses each send by name; the journal says so once, and
`state.extensions` shows it unbound. Delivery's classifier names the
platforms a plan's extensions allow, as it does for hatch words.

## 4. What exact2 changes

1. `scripts/app.schema.json` and `contract/cli/src/manifest.rs`: the
   `extensions` field.
2. The extension file's reader and the bake's checks (D3), in `contract/cli`,
   next to `native.rs`'s `data_words`.
3. The plan's extension table (D4); `plan/tables/format.json` and its digest.
4. The runner: routing a declared source or send to the native slot (D5),
   the `prefer` names and `state.extensions` (D7), the defaults of D9.
5. Each host's module build (D6): the Apple module's dispatch and
   `ExactExtension`; the web page module's dispatch; the Linux and Windows
   `Extension` trait.
6. `scripts/exact.mjs`: `exact extension new <name>`, which writes the
   directory of D1.

## 5. What stays refused

- A `module` or `extension` declaration in Contract (LLP 1024 §5).
- An agent operation per extension (LLP 1069.007 D1; `DEFERRED.md`'s ten).
- A new host command per extension; an extension acts through sends.
- An extension that writes Contract state or dispatches (LLP 1075.003 §3.4).
- Native code over the network, or a roster that changes on iOS after
  review (LLP 1024 D3).
- A value stream: a live source invalidates and is asked again (LLP 1016.002
  D1); streams wait for LLP 1016.000.

## 6. Spike on today's seams

Before a ruling, a spike shows the design on iOS and the web with no exact2
change: an app outside exact2 (made with `exact new`) that lists two
extensions and has no data code of its own.

- **`battery`** (LLP 1069.000 D4's own example), implemented on iOS: it reads
  `UIDevice.batteryLevel` and `batteryState` with
  `isBatteryMonitoringEnabled` set, and announces `battery.battery` on
  `batteryLevelDidChangeNotification` and `batteryStateDidChangeNotification`
  ([UIDevice](https://developer.apple.com/documentation/uikit/uidevice)).
  Under the agent its substitute starts at 50% and loses 1% a minute on the
  session clock, announcing each step.
- **`attitude`** (pitch and roll in tenths of a degree, the declaration the
  out-of-tree port's level app reads), implemented on the web: it reads
  `deviceorientation` (`beta`, `gamma`;
  [DeviceOrientation Event](https://w3c.github.io/deviceorientation/)) and
  announces `attitude.attitude` at most ten times a second, and only after a
  change of half a degree, so a live source is not an animation channel (LLP
  1075.003 §3.3). Where the browser asks for access
  (`DeviceOrientationEvent.requestPermission`, which iOS Safari and current
  Chrome have), events start only after the send `requestAttitude()`, made
  from a tap. Under the agent the device lies level and access is granted.
- **What exact2 would do (D3, D5, D6, D9)** the spike does with code it
  generates from the files: a TypeScript data source (LLP 1027) that
  forwards each source and send to `native.later` and watches live topics;
  an `ExactModule` (LLP 1067.000) and a page module that dispatch on
  `"extension"`; and, on a platform with no implementation of an extension,
  its declared `default`.
- **Agent mode (D7)** is each extension's own substitute (LLP 1067.000 D6),
  since `prefer` names need the runner change.
- **Run and drive:** the app's `bun exact.mjs ios --run` (a simulator),
  `ios --device --run` (an iPhone) and `agent ios`; `test web`; and, outside
  the agent, `bun exact.mjs web` in Chrome, whose orientation the DevTools
  protocol sets (`DeviceOrientation.setDeviceOrientationOverride`).

**Results** (2026-10-13, exact2 7fa397d4e). Both paths work end to end, on
a simulator, a real iPhone and Chrome, with no exact2 change.

| Where | Run | What it showed |
|---|---|---|
| iPhone 18 Pro simulator, iOS 27.0 | under the agent | `battery`: "50%", "unplugged": the journal shows `query power: battery`, `POST exact-native:` and an HTTP 200 answer of 33 bytes (`{"level":0.5,"state":"unplugged"}`). After `clock +60000` the extension announced `battery.battery`, the runner asked again, and the tree showed "49%"; after a further `clock +120000`, "47%". A request in flight when the topic changed landed first and was then asked again (LLP 1016.002 D4). `attitude`, not implemented on iOS: its default, "No motion data". |
| iPhone 18 Pro simulator, iOS 27.0 | outside the agent | `battery`: "Unknown": the simulator reports no battery (`batteryLevel` -1, state unknown). |
| iPhone 16 Pro Max, iOS 26.7.1 | outside the agent | `battery`: "90%", "charging": the real values, the same as the phone's status bar (read by argent's XCUITest runner). |
| Web (JS target) | `bun exact.mjs test web`, under the agent | `battery`, not implemented on the web: its default, "Unknown"; `attitude`: the substitute, "pitch 0.0°, roll 0.0°" (1 test passed). |
| Chrome (desktop), JS target | outside the agent | Before access, `attitude` answered its default and the view showed "Enable motion". A click sent `requestAttitude()` ("access: granted") and the view showed the overridden orientation, "pitch 20.0°, roll 5.0°". A new orientation (-15°, 30°) showed at once, with no reload. A change of less than half a degree (-15.3°, 30.2°) reached the page but was not announced, and the view kept "-15.0°, 30.0°". |

- Rechecked on exact2 d15a3f8ed (2026-10-14): `test web` passed, and under
  the agent the simulator showed "50%", then "49%" after `clock +60000`, and
  `attitude`'s default. The app needed only the Contract's new spellings
  (`for`, `if`, `x.toFixed(1)`); the extensions and the glue did not change.
- The app's own data code for the two extensions is none: `app.ts` only
  spreads the generated sources. The generated glue for two extensions is
  24 lines of TypeScript, 56 of Swift and 26 of JavaScript per app, which
  D5, D6 and D9 would move into exact2.
- What the glue cannot do, and the runner change would:
  - check a reply against the declaration; the generated TypeScript casts
    it to the declared type (D5);
  - check the app's fields against the file at bake (D3); only the version
    and the files are checked;
  - a `prefer` name for the value; under the agent only the extension's own
    substitute answers (D7);
  - the `app.json` field: the manifest's schema refuses an unknown key, so
    the list is in a separate `extensions.json` (D2);
  - hold a permission request for the agent to answer: under the agent
    `requestAttitude()` answers "granted" at once (D7, point 3).
- Not checked: a change of battery state on the iPhone (the cable unplugged
  with the app open); `attitude` in a phone's browser (not run); other
  platforms.

## 7. Questions for Charlie

1. **Take or waiver.** Is this worth a slot, and what comes off the doing-list
   for it, or is it waived?
2. **Where extensions live.** The app's `extensions/`, then exact2's? A
   separate repository of shared extensions?
3. **Optional extensions (D9).** Refuse at build by default, or default to
   optional?
4. **A host's implementation and the app's (D6).** If both exist, which
   wins, or is it refused?
5. **Versions.** One integer per file (D1), or a version per source?
6. **Fire-and-forget actions.** Is a send enough, or does an action like
   `haptic` need an extension command beside `HOST_COMMANDS`?
