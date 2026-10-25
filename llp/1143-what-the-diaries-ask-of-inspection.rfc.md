# LLP 1143: What the diaries ask of inspection

**Type:** RFC
**Status:** Draft r2, 2026-10-25. r1 was reviewed blind by GPT-6 Astra (max), Claude Fable 5.1 and Grok 4.7 (xhigh) (`llp/reviews/rfc-2026-10-25-1143-r1.{astra,fable,grok}.md`); Muse was not available on this machine. Changes are in §R.
**Systems:** the agent driver (`scripts/agent.mjs`, `agent-inspect.mjs`, `agent-launch.mjs`); the iOS agent (`host/apple/Sources/ExactKit/IOS/AgentIOS.swift`) and the Apple navigation and presentation hosts; the web JS target's compiler (`host/web-js/src/main.rs`, `emit.rs`); the game engine's JSON codec (`game/engine/src/data/json.rs`); `scripts/verify.mjs`; `scripts/cargo-dep-info.mjs`
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-25
**Related:** LLP 1012, 1012.001.000 (trustworthy drives); LLP 1079 (perf, traces); LLP 1035.002, 1035.005 D3 (the development source map); LLP 1046.001 (a game's agent interface); LLP 1116.003 D8 (`check`), 1116.005 (verify); LLP 1142 (what a development build tells an agent, in progress, which this document does not overlap)

## R. Review, and what r2 changed

| Change | Raised by | Where |
|---|---|---|
| `verify` already has `verify.accept` (kind and testId), and its report already groups findings by rule. A delta against the last run would call a finding on a screen that was not reached "fixed", would merge rows that differ only in a number, and would normalize the noise rather than remove it. D5 shrinks: the closing line says which kinds make up the count, and names `verify.accept` when one kind dominates. The run-to-run delta is dropped. | all three | D5 |
| The dep-info refusal is mostly a false refusal: Cargo writes absolute paths, and `/tmp` against `/private/tmp` fails a string comparison. The comparison is fixed, and the message names the directories searched as well. | Fable | D6 |
| `simulator()` already waits on `simctl bootstatus`, so the simulator's boot state says nothing at the refusal. The refusal reports whether the app's process runs, and saves a screenshot of the display. | Fable | D6 |
| `HatchDiagnostics.measuring` is a perf window, not "a development build". D2 is gated on `HatchDiagnostics.available` (`!ExactEnv.productionBake`). | Fable | D2 |
| Under the agent's frozen timing, pushes and sheets are presented unanimated, so a transition in flight is rare. The "moving" cause becomes "presenting": a push or presentation not yet complete, animated or not. Its next step is written only after the cited failures are reproduced. | Fable | D1 |
| A suspended app cannot answer the next request, so a refusal from the app cannot cure the 120 s hang. Under the agent, `openURL` to the web, mail and the phone is journaled and not opened, as Settings already is. Whatever still leaves (a real share extension) is pushed to the driver as the scene resigns, and the driver fails the op it is waiting on. | Astra, Grok | D1 |
| The causes are read from the observation `state.navigation` and `clock settle` already use (its transition phase, which covers UIKit's pop before `willShow`), not from a narrower test of `transitionCoordinator`. | Grok | D1 |
| The production web build compiles from a baked `.plan`, which carries no map. D3 covers builds from a Contract: the dev loop, the agent, `verify`, `test`. | Astra | D3 |

## 1. The question

Every authoring diary the diary endpoint holds was read: 264 submissions from
2026-10-04 to 2026-10-25, about 400 sub-diaries, with the local DIARY.md files of
the long Bluesky ports. Each was searched for the moments an agent could not see
what its app was doing or why. This document proposes the changes those moments
ask for that are not already built, and that LLP 1142 is not building. LLP 1142
covers saving a trace without a person, traces readable on another machine,
`state.failures` and failure lines in the platform's log, late-frame phases, `perf`
handler counts, the `lower-key-position` warning and the exit-status pitfall.
Nothing below touches those.

The corpus is lopsided. About 60% of its sub-diaries are one Windows simulation
game project (Codex GPT-6), about 25% are iOS sports and browser apps (Codex
gpt-6.1-sol, T3 Code), and the rest are web, macOS and iOS apps, with the Bluesky
ports the deepest. Counts below are distinct diaries; a long diary that hit one
thing many times counts once, and says how often.

## 2. What the diaries keep

These are praised across families of apps and are not to be traded away by
anything here:

- The driver's own test-ID addressing. Where external XCTest and accessibility
  tooling marked visible controls covered, captured the wrong display or stalled
  (14 diaries), "the framework's test-ID driver worked."
- Holding and releasing fetch replies, and the clock kept apart from data
  (`clock data`, `fail fetch`): deterministic loading states (9 diaries).
- Refusals that name the fix: the compiler's diagnostics, the analyzers
  (`analyze-call-stale-read`, `analyze-read-after-assign`), the stale-build
  receipt naming the changed files, the dep-info refusal "as the diagnostic
  directs".
- `state` as a diagnosis tool, `layout` beside a reference DOM, `screenshot
  compare`, and a game's save and restore at every tick (about 35 sub-diaries).

## 3. What is already answered

Several asks are on main already; a diary that predates them, or did not find them,
asked again. They are not proposed here:

- A screenshot under agent chrome says the bars are authored (`chromeNote`), and
  `--chrome platform` shows UIKit's in drives and in tests (9 diaries asked for
  native chrome evidence; none used the flag).
- `layout` gives a scroller's range and port (`rx`, `ry`, `port`) with UIKit's
  adjusted insets.
- `tap <target> at <x> <y>`, a refused tap naming what covers its middle and the
  timers pending, `screenshot … over … every …`, `screenshot … compare`, `layout`
  agreement across hosts, images in `clock settle`, `type` no longer echoing its
  value, `reachend` refused by the compiler.
- `World::span` and `perf world:world` for a game's CPU stages (14 Windows diaries
  hand-wrote these probes before 2026-10-18).

## 4. Decisions

### D1. A miss says why

**Asked by:** 6 diaries (iOS tap during a push or a sheet, 25 min and 10 failing
runs in one; a tap that opened Safari and hung the next `clock` for 120 s; a
sheet over a sheet whose buttons were "not on screen").

On iOS a target that has no view in a window is refused as `no view N on screen`
(`AgentIOS.swift`, two sites), whatever the cause. The refusal says which of
four causes it is, read from what `state.navigation` already observes (the top
route, the stack, the top sheet, the transition phase, which is `in-progress`
from a push's first frame, and from UIKit's removal of a popped controller,
through `didShow`) and from whether the runner still has the node:

- **in transition**: the phase is `in-progress`. The reply quotes the phase,
  the route and the sheet. Under the agent's frozen timing these transitions
  are unanimated, so this is a completion a run-loop turn behind the tap's
  reply. Its next step is written after two of the cited failures are
  reproduced.
- **covered**: no transition, and the view's route or sheet is not the top one:
  which route or sheet is on top.
- **unmounted**: the node is live in the runner but has no view (a list row
  outside its realized range): "`tap <list> into <key>` brings it in".
- **gone**: the node is not live: the refusal says so.

The app leaving the foreground is answered in two parts. A suspended app cannot
refuse anything, so the first part keeps it from leaving. Under the agent,
`openURL` to the web, mail and the phone (`http`, `https`, `mailto`, `tel`) is
journaled and not opened, as Settings and a declared scheme already are. The
drive keeps its app, and `logs` shows the URL. The second part covers what
still leaves (a real share extension, which a drive asks for). As the scene
resigns active, the host pushes one unsolicited line to the driver. The driver
fails the op it is waiting on at once with a note naming the cause, instead of
waiting out its 120 s reply bound.

This is the same refusal, made specific. It adds no operation.

### D2. The host says what it did, in `logs`

**Asked by:** 12 diaries. In 7 iOS diaries and 4 incidents of the deepest Bluesky
port, the root cause was found only by adding `NSLog` to exact2's own Swift and
rebuilding: UIKit's Back removing a controller a frame before `willShow`, which
control held focus when autofocus was skipped, the status bar style under a dark
trait, a presentation finishing after the agent's tap replied. Each round of that
costs about 2 minutes; the incidents cost 15 to 60 minutes each. "The framework's
session log is not on the device console."

A development build's Apple host writes a line to the runner's journal
(`Runner::log`, which `logs` already reads, stamped with the clock) for a fixed
set of host events:

- navigation: a push, a pop, `willShow` and `didShow` with the route each names,
  animated or not, and who started it (the app, UIKit's Back, the edge swipe).
- presentation: a sheet, popover, alert or full-screen cover presented and
  dismissed, at its start and end, and a dismissal by the person (a swipe down).
- focus: the first responder changing, with the node, and an `autofocus` that
  was not applied, with why.
- keyboard: shown and hidden, with its frame.
- appearance: the root controller's status-bar style and interface style when
  either changes.

Each line starts `host:` so it reads apart from the runner's. The same lines go
to the unified log (subsystem `dev.exact`, category `host`). A run a person
drives by hand, or one under real HID touches, can then be read with `log
stream`. These lines are separate from LLP 1142's `journal` category, which
carries failure lines.

All of it is gated on `HatchDiagnostics.available` (`!ExactEnv.productionBake`),
so a production build writes nothing, and every development build does,
driven or not.

This is not a causal trace (DEFERRED: refused). It records events, never edges
between them. The set is fixed: a new kind of line is a new row in that set and
a line in this document, not a configurable stream. macOS shares the
presentation and focus rows where its host has the same events. Linux and the
web are out of scope here. The web's page has its own console, and Linux has no
native navigation.

### D3. A web-target refusal names its source line

**Asked by:** the Bluesky port (5 incidents, 10 min each), and 1 sports app.
`'the reachend event is not in the JS target', at a node number, not a source
line.`

`exact-web-js js` refuses what the JS target lacks as `node {i}: …` (15
sites in `emit.rs` and `style.rs`). When the input is a Contract, the compiler
already has the development source map (`SourceMap`). It already resolves a
native bake's refusal through it to the file, line and component call-site
chain (`SourceMap::bake_error`).

An emit refusal that names a plan node goes through the same resolution, and is
printed as `contract build` prints an error, with its excerpt. This covers every
build from a Contract: the dev loop, the agent's web host, `verify` and `test`.
When the build compiled no map (no `--sites`), it compiles one on the failing
path only. A refusal without a node keeps its current text, and so does a
build from a baked `.plan` (the production web root), which has no map.

### D4. A summary looks like a summary

**Asked by:** 9 diaries (Windows game), each 2 to 5 minutes. One test failed
because it mistook a summary for missing state: "summarized a packed integer
selection list as a byte count and hash … an initial UI assertion mistook that
for absent runtime state."

A game's world state, as `state <canvas>` and a JSON save print it, encodes a
numeric bulk (`Bulk::U8/U16/U32/F32`) as `{"bytes":N,"hash":"0x…"}`
(`game/engine/src/data/json.rs`). It changes to:

- **Small bulks inline.** 64 elements or fewer are written as a numeric array:
  the form the JSON decoder already accepts for a bulk ("supply a numeric
  array"), so the output reads back.
- **Larger bulks marked.** Over 64, the summary says what it is:
  `{"summary":"u32 × 4096","bytes":16384,"hash":"0x…"}`. The `summary` key is
  how a test or an agent tells a summary from data.
- **The encoding documented.** The game README's state section shows how an
  option, an enum arm and a bulk are written, with one example each.

The hash and the binary save are unchanged, so determinism is untouched.

A canvas target that is gone (2 diaries reused a canvas id after re-entering a
scene and got a reply with no world) is not built here. It was seen only on the
Windows host and could not be reproduced on this machine, so it goes to
`QUEUE.md` with its repro.

### D5. `verify`'s closing line says what the count is made of

**Asked by:** 9 diaries: 500 to 625 standing audit problems on every run of
one app, so "these reports are not treated as a clean full-app result". Every
agent copied the total forward and read no further.

The report already groups findings by rule, and `app.json`'s `verify.accept`
already takes a finding that is the app's design. Neither reaches the closing
line, which is the line agents copy. That line now names the kinds that make up
the count, largest first:

```
verify: 517 problems (480 tap-target, 31 contrast, 6 compiler warnings; the tests passed) in 2m 10s
```

When one kind is more than half the count, the report adds a line under it:
that kind's findings by place, and how `verify.accept` takes those the design
means.

It judges nothing differently. The exit status and the count are what they
were.

### D6. Two refusals that say what they saw

**Asked by:** 1 diary (a simulator stuck on its boot logo, 10 min) and 13 diaries
(the dep-info refusal, of which 3 found its remedy did not help).

- `the app never opened its agent socket` (`agent.mjs`) adds whether the
  process `simctl launch` started is still running, and saves a screenshot of
  the simulator's display, naming the file. A boot logo, a crash alert or a
  blank window is then seen rather than guessed.
- `no matching rustc unit dep-info for X` (`cargo-dep-info.mjs`) was mostly
  refusing valid units. Cargo's dep-info holds absolute paths, and the same
  directory spelled two ways (macOS's `/tmp` and `/private/tmp`, a linked
  checkout) failed a string comparison. Paths that differ as written are now
  compared as the file system resolves them. A refusal that remains names the
  target and build directories it searched.

## 5. Not proposed

- **Windows host and build-cache friction** (window placement, occluded window
  screenshots, disk exhaustion with no cache visibility in about 21 diaries,
  `test-rust` scope and progress in about 12). These are real and are the corpus's
  largest counts, but they concern the Windows host and the game build, not
  inspection. Each goes to `QUEUE.md` with its diary count.
- **A drag's release velocity under the frozen clock, and real-touch parity**
  (the Bluesky port, 8 incidents). This is structural: the agent's clock owns the
  runner's. It needs its own document. To QUEUE.
- **Film-and-diff of every presented frame on iOS** (8 diaries hand-built frame
  diffing from screen recordings to find one-frame flashes). `screenshot … over
  … every` is the seed. Whether it captures every UIKit transition frame has to
  be measured before anything is proposed. To QUEUE with the measurement.
- **Launch phases before the engine and process memory** (7 diaries). These sit
  next to LLP 1142's phases and belong after it lands.
- **A world diff of two saves** (about 12 game sub-diaries). `prove.mjs
  --compare-saves` exists. A field-level diff is game tooling, to QUEUE.

## 6. Plan

One commit per decision, each with its tests, in this order: D3, D4, D6, D5, D1,
D2. D1 starts by reproducing two of its cited failures on a simulator. The web, game and script changes are tested by their crates' and scripts'
tests. D1 and D2 are tested by an iOS drive of a fixture app that pushes, presents
and focuses (`apps/shelf`, or a fixture beside the existing agent tests), checked
on an iOS simulator. The six checks run before each lands. `contract-difftest
quick` is not needed: nothing here touches `contract/`, `runner/`, `plan/` or
`semantics/`. D3 calls the map from `contract/cli` without changing it.

## Open questions

1. D2's lines go to the unified log as well as the journal. Should the
   runner's own journal lines go there too in a development build? That
   generalizes LLP 1142's failure path, so it waits for that to land.
2. The app behind D5's 500 findings is not in this repository. Classifying its
   standing set, and fixing the `check` heuristics that misfire on its design,
   is the real repair. That needs the app or its report, so it goes to
   `QUEUE.md`.
