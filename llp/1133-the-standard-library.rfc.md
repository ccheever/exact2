# LLP 1133: The standard library — the web's names, the platform's reach

**Type:** RFC
**Status:** Accepted (Charlie, 2026-10-18: "recs mostly look ok"), as r3 with his amendments (§0). Nothing here is built. Building starts with the first module's RFC (LLP 1133.002, geolocation), which builds the parts of LLP 1124 and LLP 1133.001 it needs; each module moves its `rules/DEFERRED.md` entry at its own ruling (§0).
**Systems:** every host's device seams (Apple, Android, web JS target, Linux, Windows; the terminal none); the runner's native slot and reserved sources; the data executors' capabilities (`storage`, `secret`, `fetch`, `crypto`); the Contract compiler's commands and facts (`contract/syntax` `HOST_COMMANDS`); the bake's grant table (LLP 1069.008) and link-by-use (LLP 1047, 1047.001); the agent's `prefer` and `pending` (LLP 1069.007); host extensions (LLP 1124); access hatches (LLP 1075.003.000.001)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-18
**Related:** LLP 1115 (write the web, ship the platform: the rule this extends from presentation to capability); LLP 1069 (the Expo survey and its sub-RFCs, the capabilities built so far); LLP 1069.007 (every capability's substitute; D1 facts and requests; D7, the paragraph each module carries); LLP 1069.008 (permissions derived from grants); LLP 1024 and 1067.000 (the app's one native module: views, `call`, `later`, topics; D9, `call`); LLP 1124 (host extensions: a declared, versioned, shared implementation); LLP 1075.003.000.001 (access hatches: the app's native code receives Exact's own objects); LLP 1027 and 1027.001 (TypeScript and Rust data code; language parity); LLP 1018 D5 (credentials stay below the data seam); LLP 1047.001 (a feature is linked only when used); LLP 1091 (Contract modules); LLP 1096, 1098, 1125 (sound, the media session, MIDI: capabilities added as runner vocabulary)
**Children:** LLP 1133.000 (the roster: every module, ranked); LLP 1133.001 (modules anyone can write: interfaces, implementations, platforms); LLP 1133.002 (geolocation, and how every module asks permission); LLP 1133.003 onward, one RFC per module, each written when its module is next to build

## Summary

Exact's **standard library** is the set of device and system capabilities
that exact2 ships and maintains: location, notifications, camera, sensors,
biometrics, purchases and the rest. It is the analogue of the Expo SDK.
This RFC says how every module, exact2's or anyone's, is designed; LLP
1133.000 says which modules exact2 builds and in what order; LLP 1133.001
says how modules are packaged so that anyone can write one or add a
platform to one.

The design is LLP 1115's, carried from presentation to capability. Each
module has three tiers, and an app uses whichever it needs, call by call:

| Tier | What the author writes | Where it runs | UI's equivalent |
|---|---|---|---|
| **1. The shared words** | the web platform's names and semantics where the web has the capability (`getCurrentPosition(enableHighAccuracy=true)`), or the words the module's platforms share | every target that has the capability | `button`, `select`, `dialog` |
| **2. The platform's words** | the platform framework's own names, on the same module, under the platform's key: `apple.activityType="fitness"`, `android.quality="balancedPowerAccuracy"` | that platform; elsewhere the words are inert | `appearance="native"`, `buttonStyle`, platform `data-*` words |
| **3. Access hatches** | the app's own native code, handed the module's native objects (the `CLLocationManager`, the `AVCaptureSession`) | that platform | an access hatch on a node or a route (LLP 1075.003.000.001) |

The rule that decides a module is done is the *ceiling test*, beside LLP
1115's *tell test*: **would a developer who knows the platform's framework
find something a hand-built app does that this app cannot, without leaving
Exact?** If so, the module has a ceiling, and tier 2 or tier 3 must remove
it.

What the author leaves unsaid is the platform's (LLP 1115). What the author
writes wins, and platform words beat shared words on their platform.

No platform is blessed by the module system. A module that works only on
three game consoles is as complete, for the targets it declares, as one
that works everywhere (D8). exact2's own library aims, eventually, at
seven targets (web, iOS, Android, macOS, Windows, Linux and Expose), filled
in as practical; that is exact2's goal for its own modules, not a rule for
anyone's.

A module is reached from Contract and from TypeScript and Rust data code
alike, through one runner-visible route (D6). The library is built one
module at a time: an RFC written when its module is next, reviewed, built
on the targets its consumer needs, driven by the agent and by hand, then
accepted.

## 0. Rulings (Charlie, 2026-10-18)

"recs mostly look ok", with these amendments:

1. **Tier 3 is access hatches** (A1), the name hooks were given on
   2026-10-06 (LLP 1075.003.000.001): a module's objects are one more hatch
   scope, beside a node, a route, the toolbar and the app (D3).
2. **Packaging** as D5 (A2); **names are imported** for every module (A3,
   D6).
3. **No blessed platforms** (replacing the proposed platform minimum, A4):
   "we don't want to make people feel like their modules are second class
   if they only work on PS5 and Switch and Xbox but don't work on web and
   ios and android." The web is special elsewhere in Exact, not in the
   module system. For exact2's own library, "a goal of *eventually*
   supporting web, ios, android, macos, windows, linux, expose", filled out
   as practical, as an internal choice (D8).
4. **Synchronous calls stay** where a platform supports them: "i don't
   think we should say 'always asynchronous' for everything just bc the web
   can't do it" (D6).
5. **Data code reaches modules** through generated bindings over one
   runner-visible route (D6), the panel's recommendation.
6. **`DEFERRED.md`:** each module moves its entry at its own ruling, not
   per wave (A5).
7. **The working set:** this RFC and LLP 1133.001 enter `llp/current/`;
   LLP 1050.000 and LLP 1101.003 leave it (A6). The roster stays out: it is
   a list this RFC links, edited wave by wave.
8. LLP 1133.000's and LLP 1133.001's questions as each records.
9. **Asking is a guideline, not a rule** (a later ruling the same day):
   "our 'never at launch' should be a guideline but not a hard rule built
   in to the platform. we should let developers do whatever they want"
   (D4, D6 rule 3).
10. **Agents may come to grant agents:** "agents will sometimes grant other
    agents permissions without humans in the loop … one of the things that
    will have to change to make the agentic world function." Every call
    carries its origin now; the delegation rule is written with its first
    consumer (LLP 1133.002 D12).

## 1. Why now

Capabilities have reached exact2 one at a time, each in the shape its first
consumer needed:

- **As runner vocabulary:** facts (`exactViewport`, `exactPage`), commands
  (`share`, `haptic`, `showNotification`, `showPicker`, the File System
  Access pickers), sounds and sequences (LLP 1096, 1125), the media session
  (LLP 1098). Each one grows `HOST_COMMANDS`
  (`contract/syntax/src/lib.rs:94`), a closed list of 43 names.
- **As data capabilities:** `storage`, `secret.keep`, `fetch` and `crypto`,
  given to TypeScript and Rust sources (LLP 1018, 1027, 1069.005).
- **As an app's own native module:** euxy's port carries 1,607 lines of
  Swift for a scheduler, CoreMIDI, a sampler and a share card (LLP 1125);
  the recorder's microphone and the photo editor's views are their apps'
  own (LLP 1067.000, 1024). Two apps that need the battery write it twice
  (LLP 1124 §1).
- **Not at all:** location, sensors, biometrics, contacts, calendar,
  purchases, background work, Bluetooth, speech, and most of what LLP 1069
  §5 ranked "not now".

The seams for a shared implementation now exist (LLP 1067.000's one
module, LLP 1124's spike of shared extensions), and Exact runs on more
kinds of target than when LLP 1069 ranked these: phones and desktops, the
terminal, and games. James's problem (LLP 1075) applies to capabilities as
much as to navigation bars: a cross-platform location API that cannot ask
for `CLLocationManager`'s visit monitoring is a framework patch waiting to
happen.

LLP 1069 §6 argued that a capability built as a host-specific API
multiplies cost, and that capabilities should take a shared shape rather
than be platform escape hatches. That stays true of tier 1. Tier 2 is not an
escape hatch: it is declared, typed, checked at bake and driven by the
agent. Tier 3 is the escape hatch, and LLP 1075.003 already built it for UI.

## 2. What exists

| Seam | What it gives a module | LLP |
|---|---|---|
| Reserved sources | a fact every host gives, checked field by field at bake, set by `prefer` | 1069.000 |
| `HOST_COMMANDS` | an action from an event handler, by the web's method name | 1069 §1 |
| Requests | a held ticket the agent answers (`pending`, `tap @t`); `exact-native:`, `exact-auth:` | 1069.007 D1–D4 |
| Grants | `device.<name> <purpose-key>` → Info.plist, entitlements, `Permissions-Policy`; no Android column yet (`runner/src/device.rs`) | 1069.008 |
| Data capabilities | `storage`, `secret.keep`, `fetch`, `crypto` in a source's answer; never a Contract value | 1018, 1027, 1069.005 |
| The app's one native module | views, `call`, `later`, topics; one instance per session; main thread | 1024, 1067.000 |
| Host extensions (Draft) | a declared source and send, an implementation per platform, composed into the module | 1124 |
| Access hatches | the app's native code receives Exact's own objects at defined moments | 1075.003, 1075.003.000.001 |
| Link by use | a capability's code is in the binary only when the plan uses it | 1047, 1047.001 |

## 3. The three tiers

### D1 — Tier 1 is the shared words: the web's where the web has them

A module's portable surface takes the web platform's names and meanings
where the web has the capability: methods, option dictionaries, events,
error codes, permission names. For location that is the Geolocation API:
`getCurrentPosition`, `watchPosition`, `PositionOptions`
(`enableHighAccuracy`, `timeout`, `maximumAge`), `GeolocationPositionError`
(`PERMISSION_DENIED`, `POSITION_UNAVAILABLE`, `TIMEOUT`)
([W3C Geolocation](https://www.w3.org/TR/geolocation/)). An author or an
agent who knows the web already knows it.

Where the web has no API, or the web's is too coarse to be the name (LLP
1077 D14's `haptic`, whose vocabulary is iOS's because `navigator.vibrate`
has none), the module takes the name its platforms share most widely and
says so as a **declared deviation**, with its reason, in its RFC. A
capability the web never has (a console's trophies, say) has tier 1 words
shared by the platforms that have it, and no web name to follow.

Tier 1 is shaped by the web, not limited by it. A feature several of the
module's platforms share and the web lacks (approximate location; the tap
on a notification delivered while the app was closed) is a **shared word**,
named once by the module in the web's style. A target without it does what
it can and says what it cannot (D8).

### D2 — Tier 2 is the platform's own words, under the platform's key

Each module may declare, per platform, words in that platform's
framework's own names: options on a tier 1 call, extra fields on a tier 1
result, and sources and sends that exist only there. They sit under the
platform's key: `apple`, `android`, `windows`, `linux`, `web`, or the key
any other host declares (LLP 1133.001 D5):

```
getCurrentPosition(
  enableHighAccuracy=true,
  apple.activityType="fitness", apple.desiredAccuracy="bestForNavigation",
  android.quality="highAccuracy", android.provider="gps")
```

A platform word is a dotted keyword argument (Charlie, 2026-10-18; LLP
1133.002 D13); a word whose value is a record takes a shape call.

- **Names are the framework's.** `CLActivityType.fitness` is
  `activityType="fitness"`; `LocationRequest.QUALITY_HIGH_ACCURACY` is
  `quality="highAccuracy"`. Each word in the module's declaration links to
  the platform's documentation. A reader who knows CoreLocation reads the
  call without the module's docs.
- **Checked at bake.** A word the declaration lacks, or a value of another
  type, is refused by name, as LLP 1124 D3 refuses a field.
- **Inert elsewhere.** An `apple` option on Android does nothing; it is the
  meaning of a platform word, not a silent failure. A tier 2 source or send
  on a platform without it answers its declared default or refuses by name
  (LLP 1124 D9).
- **Driven by the agent.** A tier 2 word that changes what the host reports
  is in the module's substitute (D7) like any other; `pending` shows it in
  the request's `args`.

So the platform words are the author's statement, and they beat the shared
words on their platform: `apple.desiredAccuracy="reduced"` wins over
`enableHighAccuracy=true` on iOS, as an authored `buttonStyle` beats the
platform's default.

*Rejected:* a separate module per platform (an `@exact/corelocation`
beside `@exact/geolocation`). Two modules for one object split its state:
a watch started by one cannot be stopped by the other, and the app's code
forks at every call that needs one platform word. Importing a platform's
own names from its implementation (`@exact/camera/apple`, LLP
1133.001 D2) is not this: both imports reach one implementation instance.

### D3 — Tier 3 is access hatches on the module's own objects

Each module names the native objects it holds and hands them to the app's
own native code at defined moments, as an access hatch hands over a node's
view or a navigation controller (`docs/reference.md`, "Access hatches"). A
module is one more hatch scope, beside a node, a route, the toolbar and
the app:

- **What, and when.** The location module hands its `CLLocationManager`
  after it creates it and before it starts updates; the camera module hands
  its `AVCaptureSession` inside a configuration block. A choice made only
  when the object is built (an `AVCaptureMultiCamSession` instead of an
  `AVCaptureSession`) is a hatch moment too, before construction. The
  module's RFC lists each object, each moment, and the platform's type.
- **What the module owns.** Each object comes with an ownership list: the
  delegate the module keeps and forwards, the properties it sets on every
  start, what the app may change. Development builds compare what the
  module owns with what it last wrote and say by name what changed, as LLP
  1075.003 §1.2 does: detection, not enforcement.
- **One module instance.** The object is the module's, so a tier 1
  `watchPosition` and the app's own region monitoring share one manager,
  one permission and one delegate.
- **The agent.** Under the agent's substitute the module may build no
  native object, and then no hatch is called; a drive tests tiers 1 and 2. A
  hatch that produces data the app shows routes it through the app's module
  as `later` or a topic, which the app's own substitute answers (LLP
  1067.000 D6).

A module whose framework has no object worth handing over (the clipboard)
declares no hatches.

### D4 — Unsaid behaviour is the platform's

LLP 1115's precedence holds for capability: **author > platform > web
default.** What a module does when the author says nothing follows the
platform's guidelines, not the web's defaults where they differ:

- A permission prompt shows the purpose from the grant (LLP 1069.008)
  where the platform's prompt can carry one. When to ask is the app's
  choice: the platforms' guidance, ask in context when the person does
  something that needs it, is exact2's recommendation, not a rule (§0; LLP
  1133.002 D9).
- `haptic("success")` is `UINotificationFeedbackGenerator`'s success on iOS
  and `HapticFeedbackConstants.CONFIRM` on Android.

The module's RFC lists each unsaid default and its source in the
platform's guidelines; where the platform's own default is "nothing" (iOS
presents no notification while the app is in front unless the app asks,
`userNotificationCenter(_:willPresent:)`), the RFC says which answer a
hand-built app gives.

## 4. What a module is

### D5 — Packaging: a host extension, or exact2's vocabulary

A standard module is one of two things:

- **A host extension** (LLP 1124), packaged as LLP 1133.001 says: an
  interface package that declares tier 1, the shared words and the
  substitute, and an implementation package per platform with that
  platform's words, composed into the app's one native module. This is the
  default. exact2's own modules are `@exact/<name>` packages in its Bun
  workspace (`packages/`), and a third party's are the same kind of npm
  package. The standard library has no private seam: every standard module
  that is an extension is proof that a third party could have written it
  (LLP 1133.001 D8).
- **exact2's vocabulary**, when the capability needs what only the runner,
  the kernel or a data executor has:
  - a tag the kernel lays out (`video`, `input type=file`), a queue on the
    session clock (sounds, sequences, MIDI out), a fact every host gives
    (`exactPage`), an event on an element (`midimessage=`);
  - a **data capability** given to a source's answer: `storage`,
    `secret.keep`, `fetch`, `crypto`. Its values never enter the plan, the
    store or the agent's state (LLP 1018 D5, LLP 1069.005 D6), so it has no
    Contract surface. Local-auth is a parameter of it
    (`secret.keep(…, requireUserPresence=)`), not a Contract call.

  These stay in the schema, the compiler and the executors, as they are
  today.

Both kinds are listed in LLP 1133.000 and meet D9. Choosing exact2's
vocabulary needs a reason in the module's RFC; the default is the
extension.

### D6 — How an app calls a module

**From Contract.** A module's names are imported (LLP 1091; LLP 1133.001
D2): tier 1 and the shared words from the interface, a platform's own
sources and sends from the implementation that declares them.

```
use getCurrentPosition, watchPosition, GeolocationPosition from "@exact/geolocation"
use captureDepth from "@exact/camera/apple"
```

A Contract call returns nothing and takes no action argument
(`type-call-value`, `type-call-action-arg`, `contract/types/src/calls.rs`),
so a value the UI shows is a source, read as any source is, with its
parameters (`resource here = watchPosition(enableHighAccuracy=true)`);
a live one announces a topic (LLP 1124 D5). A module's sends are called by
name from an action, with keyword arguments. Module names are declared by
the module's package, not by `HOST_COMMANDS`, which stops growing; the
names in it today stay where they are (delete, don't deprecate, applies to
moving them as well: a name moves only when its module's RFC rewrites it).

The app's dependency on the package is its listing (LLP 1133.001 D4); an
app outside the repo that lacks it is told the one line to add. The use
derives the interface; the target's binding chooses the implementation
that links (LLP 1047.001: "uses is derived"), and the grant the module
requires is already a line the app writes (LLP 1069.008).

**From TypeScript and Rust.** The interface's one declaration generates a
typed binding for each language:

```ts
import { getCurrentPosition } from "@exact/geolocation";

const here = await getCurrentPosition({ enableHighAccuracy: false });
return fetch(`${api}/forecast?lat=${here.coords.latitude}&lon=${here.coords.longitude}`);
```

TypeScript gets the web's names in Promise form (a declared deviation);
Rust gets a typed request and parser, the shape `exact_data::documents`
has today. The rules:

1. **One route.** Every binding sends LLP 1124 D5's body through the native
   door data code already has (`native.later`, `Request::native`). The
   runner inspects each such body (`Runner::native_work`,
   `runner/src/runner/commit.rs`, which today forwards it unread): a body
   naming a declared module gets the same `prefer`, declared default, grant
   check, journal line, `state.extensions` count and ticket as a Contract
   read. A raw body that spells a declared call is that call, so nothing
   reaches a module unseen.
2. **Each call carries its context:** the resource or mutation that made
   it, its admitted grants and its lifetime, kept across `await`. A live
   read registers its topic against that answer, and a change or a
   `prefer` re-asks it, a request in flight landing first (LLP 1016.002).
   The web's TypeScript runtime records a watch by source name and clears
   it when `answer` returns, before an `await` resumes
   (`host/web-js/ts-data.js`); the first module's slice fixes that.
3. **Any of the app's code may make a request; a tool invocation's never
   opens system UI.** A request (LLP 1069.007 D1: `getCurrentPosition` is
   one, since it can prompt) may come from an action, a resource's answer,
   a task or launch: exact2 refuses none, and asking in context is a
   guideline (§0). Every call carries its origin, and a request whose
   origin is a tool invocation (LLP 1111) or a headless session fails
   `person-required` rather than open a prompt, a sheet or capture (LLP
   1133.002 D12). The declaration says which calls are requests.
   *Amended by LLP 1133.036 §11 (Charlie, 2026-10-26: "Accept r9, build (Recommended)"):* a fetch hop or a WebSocket open
   whose destination can show a prompt is a request too (LLP 1133.036
   D6). On the web, a tool invocation never opens a prompt by any call
   exact2 can classify; a browser's own Local Network Access prompt for
   an address only the browser resolves from a name, or for a redirect a
   browser follows while exposing no permission to read, is the
   browser's, shown in the tab the person has open.
4. **Synchronous where the target has it.** An operation the declaration
   marks `sync` is answered synchronously on targets whose host has a
   synchronous path: on native hosts, the module's `call` (LLP 1067.000 D9),
   on the main thread, inside the answer's 100 ms budget, for cheap reads
   and never for work. The binding offers the synchronous form there; on a
   target without one (the web today, where module realms throw) calling it
   throws `NotSupportedError` at once, and a portable caller uses the
   asynchronous form. A Contract source marked `sync` answers at its first
   ask, with no pending frame, where the host can (LLP 1124 D5), and
   asynchronously elsewhere.
5. **One runtime owner.** A data crate never depends on an implementation's
   crate, and `app.ts` never calls `navigator.geolocation` or another page
   API a module owns (the bake already refuses `navigator` in data code,
   `js/bake/src/typescript.mjs`).
6. **Bake reads declarations, never the device.** A binding call is an
   external read: the bake keeps the placeholder (`runner/src/store.rs`).
   The build finds module use in Contract, TypeScript and Rust, so a module
   used only from data code still enters the binding, grant and delivery
   tables.
7. **Events are not values.** A live value coalesces changes, so it cannot
   promise every motion sample or Bluetooth notification. A module that
   needs ordered events declares a bounded event or batch protocol in its
   RFC, which amends LLP 1069.007. *Built (LLP 1133.004 D6, 2026-10-23):* a
   module declares an `event` heard on the app's root, held up to its bound
   and delivered once, in order, after first pixel.

**Which to use.** Read a module in Contract when the UI shows the value.
Call it from data code when the value is an input to a request or a
computation: a resource that reads another resource uses its displayed
value and does not wait (`docs/contract-for-agents.md`, "Resource inputs
are not asynchronous prerequisites"), so
`resource forecast = weather(here.coords.latitude, …)` asks once with the
placeholder, where `await getCurrentPosition()` in the source asks once
with the position.

### D7 — Every module's permission, substitute and grant

**Permission** follows LLP 1133.002 §4 (D6–D12), ruled for every module on
2026-10-18. Three authorities stay apart: what the app is built to request
(its grant line, which derives every manifest), what the platform currently
permits (a fact, `granted`, `prompt`, `denied` or `unknown`, each meaning
the same on every target), and what a given caller may do (its origin,
carried with every call). Any of the app's own foreground code may ask, and
when to ask is the app's choice; the platform's own mechanism asks, one
decision at a time; the app explains in its own UI and recovers through
`openURL("app-settings:")`; and exact2 never answers, pre-grants or fakes a
platform's prompt, while a tool invocation or a wake never opens one. Each
module's RFC gives its platforms' mapping onto those states, and amends LLP
1133.002 §4 where its permission does not fit.

Every module carries LLP 1069.007 D7's Substitute paragraph and names its
grant in LLP 1069.008 D2's table. In short:

- a fact has a `prefer` name and a launch flag; a request is held as a
  ticket the agent answers with `tap @t <choice>` or a fixture;
- the declaration says, per source and send, whether it is a fact or a
  request, independently of whether Contract reads it as a source;
- a permission's state is each module's own fact (`granted`, `denied`,
  `prompt`), and asking for it is a request;
- a production bake reaches none of it.

A module whose shape LLP 1069.007 D1 does not cover (a stream, background
or cold-start delivery) says so, and its RFC proposes the rule it needs,
which then amends LLP 1069.007.

### D8 — Every target it names, or a visible absence

**No platform is blessed.** A module declares the targets it supports, and
for those it is complete; a module for PS5, Switch and Xbox that never runs
on a phone is a first-class module, with the same packages, conformance and
absence rules as any other (LLP 1133.001). Nothing in the module system,
the bake or the agent ranks one target above another.

A module's RFC has a table of the targets it considers. Each cell is the
platform API it uses, or one of:

- **absent**, when the platform has no such capability (a barometer on a
  Mac);
- **later**, when it has one and the module has not reached it yet.

Absence is visible to the app, never a quiet no-op: a module answers an
availability fact by the web's feature-detection idiom where there is one
(`"geolocation" in navigator` is the module's `available` field), and a
call where it is absent refuses with the web's error for it
(`NotSupportedError`).

**exact2's library aims at seven targets:** web, iOS, Android, macOS,
Windows, Linux and Expose, eventually, filled in as practical. It is
exact2's own goal for its own modules, not a requirement on anyone's and
not a gate on accepting a module: a module ships when its consumer's
targets work, and every other cell of the seven reads `later` or `absent`
until it is filled. Other targets (tvOS, watchOS, visionOS, the terminal,
consoles) are welcome cells beside them. Linux and Windows cells use the
platform's own capability API where one exists
([xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/),
[Windows App SDK](https://learn.microsoft.com/windows/apps/windows-app-sdk/)).

**A capability only one platform has** (App Tracking Transparency, Screen
Time, Android's foreground services) is a **platform module**: it has no
tier 1, its whole surface is tier 2 under that platform's key. It is in
exact2's library when enough apps on that platform need it (LLP 1133.000
ranks it like any other); otherwise it is left to apps, which reach it
through tier 3 or their own module, or to whoever publishes one.

### D9 — The checklist every module RFC answers

A module RFC is not ready to build until it says:

1. **Tier 1:** the web API it follows, with a link, or the shared words
   its platforms take; every declared deviation with its reason.
2. **Tier 2:** each platform's words, linked to the platform's docs, and
   what each beats in tier 1.
3. **Tier 3:** the objects handed over, when, and the ownership list; or
   that there are none.
4. **The ceiling test:** the common uses of each platform's framework, and
   which tier reaches each, counted: a use no tier reaches is named, with
   why, and the module is done when that list holds only uses its RFC
   leaves out on purpose.
5. **Unsaid defaults** (D4), each with its source.
6. **Packaging** (D5): extension or exact2's vocabulary, and why if the
   latter.
7. **Callers** (D6): which names Contract reads, which data code calls,
   which are requests, which are `sync` and on which targets.
8. **Every target** it considers (D8).
9. **Grant and substitute** (D7).
10. **Cost:** what it adds to a binary that uses it and to one that does
    not (nothing), measured by LLP 1047's probe; nothing before first pixel
    (`rules/RULES.md`, the boot path).
11. **Consumer:** the app that uses it first, which is the module's
    acceptance drive on each target it builds.
12. **What stays out,** each until a consumer asks.

## 5. How the library is built

### D10 — The roster first, then one module at a time

LLP 1133.000 lists every module exact2's library should have, with its
tier 1 name, its platforms, what exists today, its consumer and its rank.
It is written once now, so that the library is designed as a whole, and
edited as modules land.

A module's RFC (LLP 1133.002 onward) is written when it is next to build,
not before (`rules/RULES.md`: a spec needs an implementer and a date). Each
goes through:

1. **The RFC,** answering D9.
2. **Review:** at least one blind review from another model family, aimed at
   the ceiling test and the tier boundaries (`llp-review`).
3. **Charlie's ruling,** which moves the module's `DEFERRED.md` entry with a
   take or a waiver.
4. **Build on the targets the RFC names,** with the agent substitute first
   so the drive can run before the device code exists.
5. **Drive the consumer** on each target, by the agent and by hand on a real
   device where the capability needs one (a location, a camera, a
   Bluetooth device).
6. **As built,** in the RFC, and the roster's row updated.

Modules that share a framework or a consumer may go as a small batch with
one RFC each (location and its region monitoring; notifications and the
badge).

### D11 — Existing capabilities join as they are

The capabilities LLP 1069's family, 1096, 1098 and 1125 built are modules
of the library already, on the targets the roster lists for each. They are
not written again. Each one gets tier 2 and tier 3, the D9 checklist and
further targets when a consumer meets its ceiling, through an amendment to
its own LLP.

## 6. What this amends

- **LLP 1124** becomes the seam of most standard modules. Its open review
  findings (a `sync` source on the web, `prefer extension` re-answering
  readers, an additive change without a version bump) and its questions
  2–6 are answered by its own r2, which this RFC and LLP 1133.001 feed:
  questions 2, 4 and 5 by LLP 1133.001 (packages, binding, semver),
  question 6 by D6 (a module's sends are called by name, outside
  `HOST_COMMANDS`). D5's `sync` mark stays, answered asynchronously on a
  target without a synchronous path (D6 rule 4). D7's claim that it closes
  LLP 1069.007 D5 holds once data-code calls pass the same interception
  (D6 rule 1).
- **LLP 1124 §6 and LLP 1067.000 D9** show data code reaching a module
  with hand-written `native.later` glue, which skips `prefer` and the
  permission ticket; D6's generated bindings replace that as the way to
  call a declared module.
- **LLP 1069 §6:** a platform API is acceptable as tier 2 of a module whose
  tier 1 its other platforms have; it is not acceptable as a module's only
  surface while another platform has the same capability.
- **LLP 1069.007 D7** applies to every standard module (D7).

## 7. What stays refused

- Native code over the network, or a module roster that changes on iOS after
  review (LLP 1024 D3).
- An agent operation per module (LLP 1069.007 D1).
- A module that runs app code before first pixel, or links when unused.
- A cargo feature on a core crate for a module (`AGENTS.md`, optional
  capability): a module is a separate artifact or the app's native module.
- A module that writes Contract state or dispatches outside its declared
  sends and sources (LLP 1075.003 §3.4).
- A module whose surface is tier 2 alone while another platform has the
  same capability: that capability needs shared words.
- A call to a declared module that bypasses the runner (D6 rule 1); a
  prompt or sheet opened by a tool invocation (D6 rule 3); exact2
  answering, pre-granting or faking a platform's permission prompt (LLP
  1133.002 D12).
- A secret, key or token as a Contract value (LLP 1018 D5).

## 8. What the first modules show

LLP 1133.000 ranks geolocation first (its §4): a web API with clear
semantics, a consumer (Weatherlight's "Use my location", whose forecast
already takes coordinates), an existing grant (`device.geolocation`), a
tier 2 word the consumer needs (approximate location), and a first use of
D6's data-code binding. It tests the tiers, the packages and the bindings
on a light native object. It does not test views, a heavy object's
hatches, ordered events, secrets, or delivery while the app is closed:
notifications, camera and recording, next in the order, test those, and
each amends this RFC if the design does not hold.
