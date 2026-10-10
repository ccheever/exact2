# Keyboard: a capture-phase key handler and held-modifier state (keyup, code, repeat landed in #220)

**Status:** Open
**Systems:** Contract, runner events, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/140

## Current scope

Pin selected capture spelling and DOM capture/target/bubble dispatch before implementation; check cancellation/IME/editable shortcuts. keyup/code/repeat already landed; held modifiers use keyup plus page focus, no new keyboard fact.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
A Contract `key` handler is DOM's `keydown` with `{ key, shiftKey, ctrlKey, altKey, metaKey }`. There is no `keyup`, no `code`, no `repeat`, no way to know that a modifier was released, and no capture-phase handler. This holds on every host (checked on macOS and the web). A `press` already gets the modifiers held at a click. Ask: add the missing parts of DOM's `KeyboardEvent` model so an app can show "⌘ held" hints, ignore auto-repeat, match a physical key, and see a key before the focused element.

## Why it matters
A coding-agent desktop app shows jump-number hints on list rows while ⌘ is held. It shows alternate button labels while ⌘ or Shift is held. It ignores auto-repeat so that a held ⌘W closes one panel, not five. It matches shortcuts by physical key when the layout types non-Latin letters. It lets window-level shortcuts run before a focused field takes the key. On the web each is a few lines (`keyup`, `event.code`, `event.repeat`, `addEventListener(…, true)`).
Today an app must put native key monitors (`flagsChanged`, keyDown/keyUp monitors, `isARepeat`) in its host code. These are below Contract, so agent tests cannot see them, and every check needs a person at a keyboard.

## Current behavior (exact2 4c893fef6)
| Sub-capability | Verdict | Observation |
|---|---|---|
| `keyup` | reproduced | `keyup=…` refused: `` lower-unknown-attr | `column` has no attribute `keyup` ``. Agent `type field key Meta up` runs no action (epoch stays 3 on both hosts). |
| `KeyboardEvent.code` | reproduced | `` type-unknown-field | `KeyboardEvent` has no field `code`; available fields: `key`, `shiftKey`, `ctrlKey`, `altKey`, `metaKey` `` |
| `KeyboardEvent.repeat` | reproduced | Same diagnostic for `repeat`. The agent's `key a for 1200` also gives one keydown (`inner:a outer:a` once). |
| Held-modifier state | reproduced | `key Meta down` is heard (`inner:Meta+meta`), its release is not, and no reserved source carries keyboard state (the sources are `exactViewport`, `exactPage`, `exactDelivery`, `exactSurface`, `exactTime`). An app cannot know when ⌘ is released. |
| Capture phase | reproduced | Focused input and its ancestor both have `key`: the log is `inner:b outer:b` (bubble order); no form lets the ancestor hear first. |
| `compositionend` before a chord | unverified | `compositionend` is refused (`lower-unknown-attr`). The chord-during-IME behavior was not driven: it needs a real input source (Korean 2-Set, Japanese). |
| Modifiers on `press` | already-supported | `press` takes a `MouseEvent`; `tap send modifiers Meta` logs `press:meta`, plain `tap send` logs `press:plain` (web and macOS). |

The game-canvas key path on macOS already reads `code`, down/up and `isARepeat` (`host/apple/Sources/ExactKit/Mac/CanvasInputMac.swift:78,89`); element `key` handlers do not get them.

## Expected behavior
DOM's `KeyboardEvent` (Chrome), on web, macOS, iOS/iPadOS with a hardware keyboard, and Linux:
- **Proposal: `keyup` handler.** Same payload and bubbling as `key` (keydown); fires on release, including a modifier's release (`key: "Meta"`).
- **`KeyboardEvent.code`** (`"KeyB"`, `"Digit1"`, `"MetaLeft"`): the physical key, independent of layout and input source.
- **`KeyboardEvent.repeat`**: `true` on auto-repeat keydowns.
- **Held modifiers.** With `keyup` an app can track them itself. The web also resets on window `blur`; a `blur` on the root window, or a proposal of a reserved fact (`exactKeyboard` with `shiftKey`, `ctrlKey`, `altKey`, `metaKey`, reset on window blur), would cover keys released while the window was not key.
- **Proposal: capture phase.** A handler form (for example `keycapture=…`) that runs on ancestors outermost first, before the focused element, like `addEventListener("keydown", f, true)`; `stopPropagation()` in it stops the key from reaching the target.
- **`compositionend`.** When a chord arrives while an IME is composing, composition ends first, then the chord's `key` is delivered, as Chrome does.
- **Agent.** `type <id> key <Name> for <ms>` delivers auto-repeats at the platform rate, and `up` delivers a `keyup`.

## Reproduction
Minimal app (`app.contract`; `app.ts` has no sources):
```contract
component X25Keys
  state log = ""
  state draft = ""
  action outer(k: string, e: KeyboardEvent)
    log = `${log} outer:${k}${e.metaKey ? "+meta" : ""}`
  action inner(k: string, e: KeyboardEvent)
    log = `${log} inner:${k}${e.metaKey ? "+meta" : ""}`
  action edit(v: string)
    draft = v
  action send(e: MouseEvent)
    log = `${log} press:${e.metaKey ? "meta" : "plain"}`
  view
    column testId="root" padding=20 gap=8 key=outer
      input value=draft input=edit key=inner aria-label="Field" testId="field"
      button "Send" press=send testId="send"
      text log testId="log"
```
`app/refused.contract` uses `e.code`, `e.repeat`, `keyup=` and `compositionend=` (4 diagnostics above).

| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x25-keyboard-facts`, paste files, `contract build --json` | – | `[]` | `[]` |
| 2 | `contract build refused.contract --json` | – | 4 refusals | compiles |
| 3 | `agent <host> "type field key Meta down" "type field key Meta up"` | macOS 26.6 / Chrome | `inner:Meta+meta outer:Meta+meta`; nothing on up | a keyup for Meta |
| 4 | `"type field key b"` | macOS / Chrome | `inner:b outer:b` | with capture: outer first |
| 5 | `"type field key a for 1200"` | macOS / Chrome | one `inner:a` | repeats with `repeat: true` |
| 6 | `"tap send modifiers Meta" "tap send"` | macOS / Chrome | `press:meta press:plain` | same (supported) |

### Evidence
The files are attached at the end of this issue, in folded sections.
- `evidence/X25/repro.log`: revision, commands, compiler and vocab output, both hosts' agent JSON and final `tree` log text.
- `evidence/X25/app/`: `app.contract`, `app.ts`, `app.json`, `refused.contract`.

Final log text, identical on web and macOS:
```
Text#5 [log] " inner:Meta+meta outer:Meta+meta inner:b outer:b inner:b+meta outer:b+meta inner:a outer:a press:meta press:plain"
```

## Acceptance criteria
- `refused.contract` compiles (with the chosen names) and the app's handlers receive `code`, `repeat` and `keyup` on web, macOS, iOS (hardware keyboard) and Linux.
- A conformance case against Chrome: down, auto-repeat, up of `b` with Meta held gives equal `key`, `code`, `repeat` and modifier values and the same handler order (capture, target, bubble).
- Agent: `type field key Meta up` runs the `keyup` action; `key a for 1200` gives repeats.
- AppKit test: during IME composition, a ⌘ chord ends composition before the chord's `key` handler runs.

## Notes
- Not tested: IME composition with a real input source; iOS and Linux hosts.
- Decision needed: whether held-modifier state is only `keyup` plus window `blur` (the web's way) or also a reserved fact; and the capture-phase spelling.
- Related: separate issues for key equivalents under a non-Latin input source and for text-editing primitives touch the same key path.

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef65f1bf8d6e4be6df9265e2ae49790637 (2026-10-05), macOS 26.6.2, Chrome via bun exact.mjs agent web

$ bun scripts/exact.mjs new target/repro/x25-keyboard-facts   # then app/app.contract, app/app.ts
$ bun exact.mjs contract build app.contract --json
[]

$ bun exact.mjs contract build refused.contract --json   # app/refused.contract
type-unknown-field | `KeyboardEvent` has no field `code`; available fields: `key`, `shiftKey`, `ctrlKey`, `altKey`, `metaKey`
type-unknown-field | `KeyboardEvent` has no field `repeat`; available fields: `key`, `shiftKey`, `ctrlKey`, `altKey`, `metaKey`
lower-unknown-attr | `column` has no attribute `keyup`
lower-unknown-attr | `input` has no attribute `compositionend`

$ bun exact.mjs contract vocab keyup | compositionend | keydown
`keyup` is not a tag or an attribute
`compositionend` is not a tag or an attribute
`keydown` is not a tag or an attribute
key: handler attribute
  event key

$ bun exact.mjs agent web "type field key Meta down" "type field key Meta up" "type field key b" "type field key Meta+b" "type field key a for 1200" "tap send modifiers Meta" "tap send" tree
{"typed":2,"key":"Meta","phase":"down","delivery":"platform","target":"field","carrier":"web","mode":"agent","clock":0,"epoch":3,"incarnation":1}
{"typed":2,"key":"Meta","phase":"up","delivery":"platform","target":"field","carrier":"web","mode":"agent","clock":0,"epoch":3,"incarnation":1}
{"at":[210,29],"typed":2,"target":"field","delivery":"platform","carrier":"web","mode":"agent","clock":0,"epoch":6,"incarnation":1}
{"at":[210,29],"typed":2,"target":"field","delivery":"platform","carrier":"web","mode":"agent","clock":0,"epoch":8,"incarnation":1}
type field {"key":"a","phase":"down"}
{"typed":2,"key":"a","phase":"down","delivery":"platform","target":"field","carrier":"web","mode":"agent","clock":0,"epoch":10,"incarnation":1}
clock +1200
{"clock":1200}
type field {"key":"a","phase":"up"}
{"typed":2,"key":"a","phase":"up","delivery":"platform","target":"field","carrier":"web","mode":"agent","clock":1200,"epoch":10,"incarnation":1}
{"at":[210,55],"tapped":3,"target":"send","delivery":"platform","carrier":"web","mode":"agent","clock":1200,"epoch":11,"incarnation":1}
{"at":[210,55],"tapped":3,"target":"send","delivery":"platform","carrier":"web","mode":"agent","clock":1200,"epoch":12,"incarnation":1}
epoch 12 · incarnation 1 · clock 1200 ms · 5 nodes
View#1 [root] (key)
  TextInput#2 [field] value="b" label="Field" (input, key)
  Pressable#3 [send] [focused] (press)
    Text#4 "Send"
  Text#5 [log] " inner:Meta+meta outer:Meta+meta inner:b outer:b inner:b+meta outer:b+meta inner:a outer:a press:meta press:plain"

$ bun exact.mjs agent macos "type field key Meta down" "type field key Meta up" "type field key b" "type field key Meta+b" "type field key a for 1200" "tap send modifiers Meta" "tap send" tree
{"typed":2,"key":"Meta","target":"field","delivery":"platform","carrier":"macos","mode":"agent","epoch":3,"incarnation":1,"clock":0}
{"value":"","key":"Meta","typed":2,"target":"field","delivery":"platform","carrier":"macos","mode":"agent","epoch":3,"incarnation":1,"clock":0}
{"value":"b","key":"b","typed":2,"target":"field","delivery":"platform","carrier":"macos","mode":"agent","epoch":6,"incarnation":1,"clock":0}
{"value":"b","typed":2,"key":"b","target":"field","delivery":"platform","carrier":"macos","mode":"agent","epoch":8,"incarnation":1,"clock":0}
type field {"key":"a","phase":"down"}
{"value":"ba","typed":2,"key":"a","target":"field","delivery":"platform","carrier":"macos","mode":"agent","epoch":11,"incarnation":1,"clock":0}
clock +1200
{"epoch":11,"clock":1200,"incarnation":1}
type field {"key":"a","phase":"up"}
{"typed":2,"delivery":"platform","phase":"up","target":"field","carrier":"macos","mode":"agent","epoch":11,"incarnation":1,"clock":1200}
{"delivery":"platform","tapped":3,"at":[210,55],"target":"send","carrier":"macos","mode":"agent","epoch":12,"incarnation":1,"clock":1200}
{"delivery":"platform","tapped":3,"at":[210,55],"target":"send","carrier":"macos","mode":"agent","epoch":13,"incarnation":1,"clock":1200}
epoch 13 · incarnation 1 · clock 1200 ms · 5 nodes
View#1 [root] (key)
  TextInput#2 [field] value="ba" label="Field" (input, key)
  Pressable#3 [send] [focused] (press)
    Text#4 "Send"
  Text#5 [log] " inner:Meta+meta outer:Meta+meta inner:b outer:b inner:b+meta outer:b+meta inner:a outer:a press:meta press:plain"

# docs/contract-grammar.md:968-971: KeyboardEvent is { key, shiftKey, ctrlKey, altKey, metaKey }
# docs/contract-grammar.md: reserved platform sources are exactViewport, exactPage, exactDelivery, exactSurface, exactTime (no keyboard state)
$ grep -n "isARepeat\|\"code\"" host/apple/Sources/ExactKit/Mac/CanvasInputMac.swift   # the game-canvas key path already carries code/down/repeat
78:            if down && event.isARepeat { return true }
89:        view.canvases?.input(view, ["t": "key", "code": code, "key": key, "down": down, "repeat": event.isARepeat], timestamp: event.timestamp)
105:        view.canvases?.input(view, ["t": "key", "code": code, "key": KeyCodes.key(code), "down": down, "repeat": false], timestamp: event.timestamp)
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
component X25Keys
  state log = ""
  state draft = ""
  action outer(k: string, e: KeyboardEvent)
    log = `${log} outer:${k}${e.metaKey ? "+meta" : ""}`
  action inner(k: string, e: KeyboardEvent)
    log = `${log} inner:${k}${e.metaKey ? "+meta" : ""}`
  action edit(v: string)
    draft = v
  action send(e: MouseEvent)
    log = `${log} press:${e.metaKey ? "meta" : "plain"}`
  view
    column testId="root" padding=20 gap=8 key=outer
      input value=draft input=edit key=inner aria-label="Field" testId="field"
      button "Send" press=send testId="send"
      text log testId="log"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X25 Keyboard Facts",
  "short_name": "X25 Keyboard Facts",
  "id": "com.example.x25-keyboard-facts",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x25-keyboard-facts",
    "name": "X25 Keyboard Facts"
  },
  "host": {
    "ios": {
      "minimumOS": "17.0",
      "deviceFamily": [
        "iphone",
        "ipad"
      ]
    },
    "macos": {
      "minimumOS": "14.0",
      "window": {
        "width": 900,
        "height": 700
      }
    },
    "web": {}
  },
  "deploy": {
    "store": {
      "web": "0",
      "macos": "0",
      "ios": "0",
      "linux": "0"
    }
  }
}
````

`app.ts`

````ts
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.x25-keyboard-facts';
export const grants = '';

const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
````

`refused.contract`

````contract
component Refused
  state log = ""
  action down(k: string, e: KeyboardEvent)
    log = e.code
  action down2(k: string, e: KeyboardEvent)
    log = e.repeat ? "r" : ""
  action up(k: string)
    log = k
  action composed(v: string)
    log = v
  view
    column key=down keyup=up
      text "x" key=down2 tabindex=0
      input value=log compositionend=composed aria-label="F"
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T08:38:07Z

## Decision needed for the rest
#220 landed `keyup`, `KeyboardEvent.code` and `.repeat` on web, macOS, iOS and Linux. A real ⌘B gives the same events as Chrome on macOS. #229 covers what needs no decision:
- On macOS and iOS, a ⌘ chord during an IME composition commits it, then reaches the `key` handlers. Chrome delivers the chord mid-composition, but an AppKit field reports composed text only once it is committed, so committing first gives the handler the value Chrome's handler sees.
- A modifier released mid-composition is a `keyup`.
- Held modifiers are tracked the web's way, with `keyup` plus `exactPage().hasFocus` from #219, as the guide now shows.

**[Design]**: an accepted LLP decides otherwise, or the API has to be chosen first. It needs a ruling before implementation.

**Open:**
- **1.** A capture-phase key handler: DOM's `addEventListener("keydown", f, true)`. Contract has no capture form for any event, so the spelling is new vocabulary. The issue proposed `keycapture=…`.
- **2.** Whether held-modifier state also needs a reserved fact (`exactKeyboard`), beyond `keyup` plus focus.

**Recommendation:** for 2, no new fact: the web has none, and `keyup` plus `hasFocus` covers it. For 1, admit `keycapture=` with DOM's order (capture outermost first, then target, then bubble) and `stopPropagation()`. Window-level shortcuts that must run before a focused field have no other form.

**Cost:** 1 small to medium (the runner's dispatch plus four hosts' key paths).

### ccheever — 2026-10-08T08:07:35Z

**Decision: Choose capture-phase key handling; decline a new keyboard fact.**

Keep open with the bounded scope below.

keyup, code and repeat already landed; existing keyup plus page focus covers held modifiers. A capture handler solves the remaining early-shortcut case.

Pin one spelling and DOM capture/target/bubble order before code; verify propagation/default cancellation, IME and editable shortcuts on each host. Do not rebuild the settled keyboard pieces.
