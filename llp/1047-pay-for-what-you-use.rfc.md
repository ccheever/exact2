# LLP 1047: Pay for what you use — a small core, and everything else linked by the plan

**Type:** RFC
**Status:** Draft, being carried out.
- Charlie set the goal on 2026-09-24: "shrink the runtime size (LLP 1047) until a tap right after load takes the same amount of time in exact2 and react, or is faster in exact2".
- §10 records the target, the lanes and the answers to §9.
**Systems:** Build (the generated app entry, `contract::rust_entry`, the hosts' export macros); Plan (a use-set derived from plan bytes, no format change); Runner (the router and inspection behind seams); Web host (`exact-web`'s exports and dependencies, `glue.js`, `navigation.js`); Apple and Linux hosts (the same rule for their archives); Delivery (the linked set as a compatibility input); Metrics (a core byte budget, reported per commit)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-22
**Related:** LLP 1000 (the crate graph: "an embedder links the crate it wants and the linker drops the rest"); LLP 1007 §7 ("Where the bytes are"); LLP 1009 D2 (the GPU module, loaded on demand); LLP 1020 (the Apple web arm, `dlopen`ed at the first `iframe`); LLP 1024 (the module roster; an unknown tag is a bake error); LLP 1030 (the delivery adapter chosen by the generated entry, its absence established by artifact inspection; D3a, the compatibility id); LLP 1043.000 §3 D7/D8 (text flow's JavaScript executor, absent from ordinary boots); `rules/RULES.md` §Time budgets and §Agents

## Summary

Charlie, 2026-09-22: *"could we split out the markdown stuff so that only
things that use it get it? in general, can we come up with a system where
there is a very small core that is required by everything, and then
everything else is only added when necessary?"*

Today every web app ships about the same 1.1 MB of wasm, whatever it uses.
The 35-line video player carries Markdown, text flow, the router, springs
and the agent's inspection code, and Caltrain is only 64 KiB bigger. The
linker already drops code that nothing references (that is why Taffy's
layout algorithms are not on the web), but the hosts reference everything.
Every export is emitted for every app, and the core calls optional crates
behind runtime branches on prop values.

The proposal is three tiers and one rule:

- **The core** is what every plan needs to show a first frame and answer a tap.
- **A linked capability** is everything else. It is in the app's artifact if
  and only if the app's plan uses it. The compiler derives a use-set from the
  plan bytes, and the generated entry names only those capabilities, so the
  linker drops the rest.
- **A loaded capability** is large and not needed for the first frame. It is
  a separate artifact fetched on first use, as the GPU module already is.

There are no cargo features and no build matrix. The per-app entry is already
generated, and LLP 1030's delivery adapter already works this way.

In JavaScript terms, a bundler can tree-shake because it sees the import
graph. An exact2 app is a plan: data that a runtime interprets. So the linker
sees one interpreter with every branch reachable. The compiler is the only
thing that sees what the app uses, so **the compiler has to be the tree
shaker**. Unlike npm, nobody installs anything. Contract is a closed
vocabulary, so the use-set is exact.

Linking by use is necessary but not sufficient. The capabilities an app might
not use are about 18% of the video player's wasm code; the rest is the core
itself. §6 is the core diet that has to follow.

## 1. Measured

2026-09-22, at `11c8274b`, on an Apple-silicon Mac:

- **exact2 build:** `cargo build --profile web --target
  wasm32-unknown-unknown`, then `wasm-opt -Oz` with the flags
  `host/web/build.mjs` passes.
- **Compression:** gzip -9 and brotli quality 11, both through Bun's zlib.
- **React:** Vite 7.3.6 and React 19.3.0, a `useState` counter built with
  `vite build` defaults.

These are bytes only; nothing below is a timing.

| | Raw | Gzip | Brotli |
|---|---|---|---|
| React + Vite, a counter | 217 KiB | 68 KiB | 58 KiB |
| exact2 video-player (35 lines of Contract), wasm | 1,121 KiB | 474 KiB | 368 KiB |
| exact2 Caltrain, wasm | 1,185 KiB | 496 KiB | 382 KiB |
| `glue.js` + `navigation.js`, every exact2 app | 159 KiB | 47 KiB | 41 KiB |

Caltrain's wasm was 404 KiB raw (172 KiB gzip) on 2026-08-28 (LLP 1007 §7).
That is 2.9× growth in 25 days, and no budget tracked it.

### Where the video player's code goes

This comes from a build that kept its symbol names (`-g` through `wasm-opt`).
Each symbol is attributed to the first exact2 module its name mentions, so
standard-library generics instantiated for a module count toward that module.
The figures are approximate.

The "used by" column counts which of the 16 apps under `apps/` use the
capability, by a text search of their Contract, so it is also approximate.
The video player uses none of these capabilities.

| Part | ~KiB of code | Used by |
|---|---|---|
| Runner: `instance` 149, `runner` 80, `vm` 16, `request` 10, `store` 9, … | 275 | every app |
| Kernel: `id` 40, `generated` 39, `txn` 16, … | 125 | every app |
| Web host: `host` 90, `abi` 13, `batch` 7, `css` 5 | 115 | every app |
| Plan decoder and values | 86 | every app |
| Rust std not tied to an exact2 type (allocator, `fmt`, sort, `BTreeMap`, …) | 232 | every app |
| **Motion:** `property`, `transition`, `engine`, `exact_web::motion` | 72 | 5 of 16 |
| **Markdown markup:** `style`, `inline`, `segment`, `pieces` | 32 | 1 of 16 |
| **Text flow:** `geometry`, `shape`, `walker`, `exact_web::textflow` | 31 | 2 of 16 |
| **Router:** `exact_route` | 24 | 3 of 16 |
| **Inspection:** `runner::agent`, `sha2` | 19 | tooling |
| **Rust data modules:** `serde_json` via `exact-logic` | 17+ | apps with a `rust.module` |

The table covers about 1,100 KiB of code. There are also about 130 KiB of
data segments (strings, tables, the baked plan) that have not been
attributed yet.

### As built, 2026-09-24: three apps at `98d5f996` (stage 1)

- **The shipped bytes** come from `host/web/build.mjs`'s own steps: its
  bake, then `wasm-opt` with its flags. The result is byte-identical to
  `build.mjs`'s `app.wasm`.
- **The names build** is the same with rustc's `strip = "debuginfo"` and
  `wasm-opt -g`. That keeps the name section and drops DWARF. With std's
  DWARF present, binaryen skips duplicate-function elimination, and the code
  is 12% larger than shipped. Without it, the names build's code is within
  0.1% of the shipped code (4,559 functions against 4,553).
- **Attribution** follows §1's rule: a function goes to the first exact2
  module its demangled name mentions.

| `app.wasm` | Raw | Gzip | Brotli-11 |
|---|---|---|---|
| RealWorld | 1,336,082 B | 517,725 B | 390,646 B |
| Video player | 1,219,546 B | 482,282 B | 365,025 B |
| Caltrain | 1,279,484 B | 500,382 B | 376,195 B |

| KiB of code | RealWorld | Video | Caltrain | Used by these three |
|---|---|---|---|---|
| Core: runner, kernel, plan decoder, web host | 529 | 519 | 535 | all |
| Rust std and other crates | 282 | 251 | 258 | all; RealWorld's includes 32 of `serde_json` for its TypeScript values |
| Collections (`list virtualized`) | 80 | 80 | 80 | none |
| Motion | 71 | 71 | 71 | none |
| Drag: height, transform, reorder | 44 | 44 | 44 | none |
| Markdown | 35 | 36 | 36 | RealWorld (article bodies) |
| Documents: a rendered page's adoption | 23 | 23 | 23 | RealWorld, Caltrain |
| TypeScript modules | 20 | – | – | RealWorld |
| Router | 15 | 15 | 15 | RealWorld, Caltrain |
| Inspection | 14 | 14 | 14 | linked in production (§10) |
| Rust modules: `exact_logic_abi` | – | 10 | 10 | Caltrain. The video player has no Rust module, but its `auto` policy links the browser executor |
| Text flow's wasm half | 8 | 8 | 8 | none |
| Fonts | 2 | 2 | 2 | none |
| Media | <1 | <1 | <1 | the video player |

| KiB of data | RealWorld | Video | Caltrain |
|---|---|---|---|
| The baked plan | 52.5 | 3.5 | 37.0 |
| The embedded module: receipt and `app.js` (af5dfdf4) | 9.4 | – | – |
| The compat receipt | 1.2 | 1.3 | 2.1 |
| Strings: `Debug` variant names, the kernel's vocabulary, error text, JSON and CSS fragments | 53.0 | 51.5 | 51.8 |
| Tables and other constants: std's float-parsing powers (about 10), Unicode and float-formatting tables, vtables, panic locations | 59.9 | 58.2 | 59.3 |
| Total | 175.9 | 114.5 | 150.1 |

**RealWorld uses** the router, Markdown, documents, TypeScript modules and
images. It uses no motion, virtualized collections, drag, declared fonts,
text flow, video or Rust modules; the last have been off since e36fb75f.
By this attribution, linking by use can take about 205 KiB of code out of
RealWorld: collections, motion, drag, text flow and fonts. Std code that only
they reach comes out too.

## 2. Why the linker keeps it

There are three causes, and each has a place in the code.

1. **Every export is a root.** `exact_web::host!` (`host/web/src/abi.rs:678`)
   emits all of its roughly 25 `#[no_mangle]` exports for every app:
   `exact_textflow`, `exact_motion`, the five `exact_list*` exports,
   `exact_agent`, `exact_logic` and the font exports. `wasm-ld` keeps
   everything reachable from an export.

   The Apple host already groups its exports into macros (`raster_exports!`,
   `markup_exports!`), but it invokes every group unconditionally
   (`host/apple/src/abi/exports.rs:19-21`).

2. **The core branches on the plan at runtime.**
   - The web batch emitter calls `exact_markdown::pieces` whenever a text
     node carries `markup` (`host/web/src/host.rs:1158`, the function at
     `:1097`).
   - The runner's stdlib reaches `exact_route` (`runner/src/stdlib.rs:38`,
     `runner/src/runner/router.rs:249-267`).
   - Inspection hashes the plan with `sha2` (`runner/src/runner.rs:685`).

   The linker cannot know that no plan will ever set that prop or call those
   functions.

3. **Core crates depend on optional ones.**
   - `exact-web` depends on `exact-markdown` and `exact-textflow`.
   - `exact-runner` depends on `exact-route` and `sha2`.
   - `exact-kernel` depends on `exact-motion` and `exact-textflow`.
   - Every app's web crate depends on `exact-logic`.

   A type costs nothing: the kernel needs `exact-motion` for the `transition`
   row's type. What matters is which code a core path calls.

The JavaScript side already has the third tier. `loadAfterPaint`
(`host/web/glue.js:63`) fetches nine optional pieces on first use, among
them:

- text flow's executor (LLP 1043.000 D7)
- media
- list selection
- the Markdown editor
- storage requests
- GPU

Two inconsistencies remain:

- `navigation.js` is a static import of `glue.js` (`glue.js:5`). It sends
  motion (37 KiB), collections (17 KiB), arrange and drag (11 KiB), history
  (10 KiB) and Markdown rendering (3 KiB) to every page.
- Text flow's JavaScript half is loaded on demand, but its wasm half is
  always linked.

## 3. Decisions

### D1: Three tiers, and every capability is in exactly one

**Core.** This is what every plan needs to decode, boot, put up a first
frame and dispatch an event:

- the plan decoder
- the runner's VM: state, derives, actions, keyed instances, events, timers
  and the data seam
- the kernel's tree and style rows
- the host's batch emitter and CSS
- the minimal glue

A thing is not core because it is common. It is core because a plan with one
`text` and one `button` needs it.

**Linked capability.** It is statically linked into the app's artifact if and
only if the plan uses it. It costs zero bytes otherwise.

**Loaded capability.** It is a separate artifact, fetched on the web or
`dlopen`ed on native, at first use and after first pixel. Current examples:
the GPU module (LLP 1009 D2), the Apple web arm (LLP 1020), native modules
(LLP 1024), SQLite, and text flow's JavaScript executor.

A capability belongs in this tier when it is large and not needed for the
first frame. Its first use costs a request, so the tier is wrong for anything
the first frame needs.

### D2: The plan's use-set decides, and it is derived from plan bytes

`exact_plan::uses(&Plan) -> Uses` is a pure function of the plan. It finds
which tags, rows, events and stdlib functions the plan contains and maps them
to capability names.

- **Derived, not declared.** No author writes a dependency list, and the plan
  format does not change.
- **Checkable against any artifact.** Because it is a function of the bytes,
  any plan can be checked against any artifact that says what it links: a
  delivered bundle, a development reload, or an agent's `--plan`.

### D3: The generated entry links what the plan uses

The build already generates each app's entry: `OUT_DIR/entry.rs` through
`contract::rust_entry`, then `exact_web::host!`. Under this RFC the entry
names the core, plus, for each capability in the use-set, that capability's
export group and its registration.

The core reaches a capability only through a seam the entry fills, such as a
function table. It never names the capability's crate. This is the shape
Apple C ABI 4 already has for delivery: the table is null in an `L=0` app
(LLP 1030).

An app crate may declare every capability crate as a dependency. Cargo
compiles it and the linker drops it. Absence is established by inspecting the
artifact, which LLP 1030 already requires for the delivery adapter.

This is not a cargo feature and not a build matrix. Each core crate builds one
way; what differs per app is which symbols the generated entry names.

### D4: Core crates stop depending on capabilities

Each capability's host adapter moves into a crate above the core host:

- **Markdown.** `exact-web` loses `exact-markdown`. The code that turns pieces
  into the batch's `markupPieces` moves to a crate that depends on both
  `exact-web` and `exact-markdown`.
- **Router.** The runner's router functions become a table the entry supplies.
- **Inspection.** The inspection digest does the same.

This keeps LLP 1000's rule that each crate depends on strictly less than the
one above it. It also makes "the core names no capability" readable from
`Cargo.toml`: a core crate that cannot name Markdown cannot call it by
accident.

### D5: The JavaScript follows the same set

`navigation.js`'s sections become capability pieces. The build writes the
boot glue as the core plus the pieces the use-set names, into the one boot
module. The `boot` check's module count does not grow, and its bytes fall.

A piece not needed for the first frame loads through the existing
`loadAfterPaint`. Glue never assumes an export exists.

### D6: Using an unlinked capability is a named refusal, never a silent gap

- **At bake:** the use-set is a subset of the linked set by construction.
- **At boot:** the host compares a plan's use-set with its own linked set.
  It refuses a plan that exceeds the set and names the missing capabilities.
  This covers a development reload, a delivered bundle and an agent's
  `--plan`.

A missing export is never discovered at runtime.

### D7: Development links everything; production links the use-set

The dev loop restarts from a new plan in about 20 ms without rebuilding the
wasm. If the development wasm linked only what the plan used when it was
built, adding `markup="markdown"` would stall the loop on a wasm rebuild.

So the development entry links the full roster, and the production bake links
the use-set. That is two generated entries, as LLP 1030 already has (the core
entry and the adapter entry), not a feature matrix.

The smokes run on the production artifact, so a missing link is refused there
(D6) and never reaches a user's browser.

### D8: On native, the linked set is a compatibility input

On the web, the wasm and the plan ship together, so linking exactly is always
safe. A native binary outlives its plans, because bundles update it (LLP 1030).

- **A new compatibility input.** The linked set joins the compatibility id's
  inputs (`contract/cli/src/compat.rs`, next to `executors` and
  `nativeModules`). A bundle whose plan uses a capability the binary lacks is
  then a binary change, and `deploy.mjs` classifies it as one.
- **Linking ahead of use.** The manifest may name capabilities to link before
  any plan uses them, for example `"link": ["markdown"]`, so a later bundle
  can use them without a new binary. This is the lesson of native modules and
  runtime versions in Expo.

Bytes cost less on native than on the web, because there is no network on the
boot path. A native app may reasonably link more than it uses; a web app
should not.

### D9: A core budget, measured per commit, never blocking

- **The carrier.** A hello app (one counter) carries the budget: its artifact
  is the core.
- **The report.** `bun scripts/metrics.mjs` reports the core's bytes (raw,
  gzip and brotli) and each capability's marginal bytes, from a build that
  keeps names. It runs per commit on fleet hardware.
- **A new budget row.** The time-budget table in `rules/RULES.md` gains a row,
  "Core web payload, hello app: ≤ N KiB brotli". It is tracked the same way as
  the existing rows: a regression is a P0 with a name on it.
- **Not a blocking check.** A wasm build does not fit in 60 seconds, so this
  is not a sixth blocking check.
- **Proving absence.** Artifact inspection shows that an app which does not
  use Markdown contains no `exact_markdown` symbol.

## 4. The first roster

| Capability | Tier | What in the plan selects it | Today, KiB of code |
|---|---|---|---|
| Motion (springs, transitions) | linked | a `transition` row or spring | ~72 wasm, 37 JS |
| Markdown markup | linked | `markup="markdown"` | ~32 wasm, 3 JS |
| Markdown editor | loaded (JavaScript already is) | `textarea markup="markdown"` | not yet measured |
| Text flow | linked wasm; JavaScript already loaded | `wrap-flow`, `shape-outside` | ~31 wasm |
| Router | linked | `routes`, router verbs | ~24 wasm, 10 JS |
| Virtualized collections | linked | `list virtualized=true` | wasm not yet measured, 17 JS |
| Drag and arrange | linked | reorder, height or transform drag | wasm not yet measured, 11 JS |
| Rust data modules | linked | the manifest's `rust.module` | 17+ wasm |
| Fonts | linked | a declared font | not yet measured |
| Media (video, image) | linked; JavaScript already loaded | `video`, `image` | not yet measured |
| Inspection (the agent API) | see §9 Q3 | none | ~19 wasm |
| GPU canvas | loaded (already) | `canvas` with a surface | separate artifact |
| Storage (SQLite, files) | loaded (already) | storage grants | separate artifact |

Stage 1 measures the cells marked "not yet measured".

## 5. What this does not change

- There are no cargo features on core crates and no per-app forks of a host.
- Contract does not change, and authors write nothing new.
- The plan format does not change.
- Data crates are unaffected. Cargo already links only what an app's data
  crate names.

## 6. The core diet: the larger half

Suppose every capability is unlinked. By §1's attribution, the video player
would still carry about 900 KiB of raw wasm, against 217 KiB raw for React's
counter. Linking by use cannot close that gap alone.

These are leads. Each is measured before it is changed.

- **`exact_runner::instance`, about 149 KiB.** It is the largest single
  module. Much of it is generic code instantiated over the app's data type
  (`Runner<AppData>`, `Host<AppData>`). A `dyn` data seam would compile it
  once.
- **`exact_kernel::id`, about 40 KiB.** That is a lot for an id module;
  probably map instantiations.
- **`BTreeMap` and `BTreeSet` instantiations, 64 KiB.** LLP 1007 §7 already
  said the engine's maps could be vectors.
- **Text formatting on every path.** The web host emits JSON batches, so
  `core::fmt` and float printing run on every update. A binary batch would
  drop most of that.
- **About 130 KiB of data segments.** Not attributed yet; find out what they
  are.
- **The toolchain.** `build-std` with `panic_immediate_abort` removes
  panic-formatting code but needs nightly Rust. This is Charlie's call (Q4).

## 7. Order of work

1. **Measure; no behavior change.**
   - the hello app
   - a web build that keeps names
   - per-crate and per-capability bytes in `metrics.mjs`
   - a symbol-absence report

   This fills §4's "not yet measured" cells.
2. **Markdown end to end: the seam's first user.**
   - `exact-web` drops `exact-markdown`.
   - The markup adapter registers through the entry.
   - `renderMarkup` becomes a JavaScript piece.
   - The Apple `markup_exports!` is invoked by the generated entry, not by the
     host's macro.
   - `exact_plan::uses` and the boot refusal (D2, D6) land here, because
     Markdown needs them.

   It is done when the video player's artifact has no `exact_markdown` symbol
   and `markdown-stress` passes its smoke unchanged on web and Apple.
3. **The other web export groups:** text flow, motion, collections, drag,
   fonts, Rust modules and inspection.
4. **The runner:** the router and inspection move behind tables (D4).
5. **The JavaScript pieces** from `navigation.js` (D5).
6. **Native:** the compatibility input and the manifest's `link` (D8).
7. **The core diet** (§6), against the D9 budget.

Stages 1 and 2 answer the question Charlie asked. Stage 1's numbers decide
whether stages 3–7 are worth their cost, and in what order.

## 8. Alternatives considered

- **Cargo features.** These are ruled out (`CLAUDE.md`; LLP 1024's table). A
  feature is a build matrix. A feature on a core crate brings back the
  predecessor's failure, where several default layers disagreed.
- **A separate wasm for every optional thing, loaded at runtime.** Rust
  modules in wasm have no mature dynamic linking. Each module would carry its
  own copy of the shared code (allocator, std) and cost a request. That is
  right for large, late capabilities (D1's third tier) and wrong as the
  general mechanism.
- **Compile the plan to Rust instead of interpreting it.** This is Svelte's
  answer to React: no runtime, only the code the app needs. It is the far end
  of this spectrum and the smallest possible output. It would cost the 20 ms
  plan restart, delivering plans without a new binary, and the one runner
  shared by every host. It is out of scope, and it marks the limit of what
  linking alone can reach.
- **Accept the size, because wasm compiles fast.** Streaming compilation
  makes wasm cheaper than JavaScript per byte on the CPU. On a slow link,
  though, the network is the constraint. At Lighthouse's slow-4G profile
  (1.6 Mbps), 382 KiB brotli is about 1.9 s of transfer, against about 0.3 s
  for React's counter. This is arithmetic, not a measurement.

## 9. Questions for Charlie

1. **The budget number in D9.** React's counter is 58 KiB brotli. A core at
   or under that is the claim worth making, and it probably needs §6 as well
   as linking. The alternative is a staged budget: no regression first, then
   a number once stage 1 has measured.
2. **D7.** Should development link everything while production links the
   use-set? Or should development link exactly and pay for a wasm rebuild
   when an edit adds a capability?
3. **Inspection in production.** Should the LLP 1012 agent operations ship in
   production web builds? Today the web glue calls `exact_agent`
   (`glue.js:960`), and the smokes drive the built artifact through it. The
   alternative is linking inspection only into the builds the smokes and
   agents use, which would make the smoked artifact differ from the shipped
   one.
4. **Nightly Rust.** Should §6 use `build-std`, which requires nightly?
5. **The manifest's `link` field (D8).** Accept it, or classify every new
   capability as a binary change with no headroom?
6. **The working set.** It is at 15 of 15, so this document is not linked
   into `llp/current/`. Which document should leave?
7. **New apparatus.** Stage 1 adds a hello app and extends `metrics.mjs`.
   Under `rules/RULES.md` §Agents, both need a human's yes. Is it given with
   this RFC?

## 10. Being carried out (2026-09-24)

**The target is an outcome, not a byte count.** It uses the RealWorld bench
(`~/projects/realworld-react/bench`, outside this repo per NOT-DOING's take):
exact2 served against React 19.3 + Vite, on the Lighthouse mobile profile
(150 ms RTT, 1.6 Mbps, 4× CPU), pressing the `python` tag at the `load` event.

Where it starts, at 0f0541f0:

| Tap at load | exact2, idle | React |
|---|---|---|
| Tap to feed | 2.51 s | 0.21 s |
| Feed shown | 3.45 s | 1.43 s |

After a 3-second read, exact2 takes 0.30 s against React's 0.21 s.

The goal holds when exact2's tap to feed is at most React's. Both rows are
reported, with the load averages, and the tap to feed after a 3-second read
too.

**Where the time goes.** The press waits for about 391 KB of brotli'd wasm
(about 2.2 s at 1.6 Mbps), then compile and boot, then the press path, which is
0.09 s slower than React's even once the runtime is loaded. Three fronts:

1. **Start at paint.** Charlie's activation ruling (LLP 1048, 2026-09-23) was
   that "the runtime downloads as soon as the page has painted". As built, idle
   waits for `load`, about 650 ms later. The download starts at first paint.
2. **Shrink the artifact.**
   - Link by use: §3 D1–D7, stages 2–5.
   - The core diet: §6, stage 7.
3. **Boot and the press path.**
   - Streaming compile and a boot from the checkpoint with nothing serial.
   - The press path from dispatch through the module realm, the fetch, the
     commit and the batch.

**Answers to §9**, settled by the coordinator under Charlie's goal:
1. **D9's budget** is the outcome above. The per-commit byte row is reported,
   never blocking.
2. **D7:** development links everything; production links the use-set.
3. **Inspection** stays linked in production for now (about 19 KiB), so the
   smoked artifact is the shipped one. Revisit it if the target needs it.
4. **Nightly `build-std`:** not yet. If stable options can't reach the target,
   ask Charlie.
5. **D8** (native): after the web target.
6. **The working set** is at 13, so this document joins it.
7. **Apparatus:** the goal authorizes stage 1's measurement, and D9's
   per-commit byte row is part of this RFC. A hello app is added only if the
   RealWorld and video-player builds can't show the core.

**Who is doing what.**
- **The size lane:** stages 1–3 and D4, linking by use.
- **The diet lane:** §6, toolchain flags, data segments, `fmt`, maps and
  generic seams.
- **The activation lane:** start at paint, boot, the press path, the JS pieces
  (D5) and the bench loop.

Other sessions changing `host/web`, the build's entry or the core crates'
size: check `git log` for these lanes and say which items you are taking.


### As built: linking by use (the size lane)

**Stage 2, 2026-09-24: the seam, with Markdown as its first user.**
- **The use-set (D2)** is `exact_runner::uses(&Plan) -> Uses`, a pure
  function of the plan's rows.
  - It lives in the runner, not in `exact_plan`: a use is a kernel prop or
    style row, and the plan crate declares no kernel vocabulary (LLP 1004 D2).
  - A computed value counts as a use of whatever it might select.
- **The seam (D3, D4).** `exact-web` no longer depends on `exact-markdown`.
  - It reaches a capability only through `exact_web::Linked`, which the entry
    registers before every boot.
  - The adapters live above the core, in `exact-web-capabilities`.
  - `contract::web_linked` writes the entry's
    `EXACT_LINKED = linked!(…)` from the use-set.
- **The refusal (D6).** A plan that uses more than the artifact links is
  refused at boot: `boot: Unlinked("markdown")`.
- **Development (D7).** `host/web/dev.mjs` builds with `EXACT_WEB_LINK=all`,
  which links every capability. It reuses a dist only when that dist's
  receipt says so. The render host links everything too.
- **Bytes, raw and brotli-11:**
  - Video player: 1,219,546 → 1,167,590 raw; 365,025 → 347,265 brotli.
  - Caltrain: 1,279,484 → 1,227,534 raw; 376,195 → 359,245 brotli.
  - RealWorld uses Markdown: 1,336,082 → 1,339,522 raw; 390,646 → 391,840
    brotli. The increase is the admission code and inlining churn.
- **Absence.** The video player keeps none of Markdown's 91 functions, except
  one 11-byte closure, `|p| !p.is_empty()`. LLVM merged it with core's
  identical `SplitWhitespace` filter and kept Markdown's name for it.

**Stage 3, 2026-09-24: motion.**
- **The seam.** The host holds a `dyn Motion`, which is `Still` (no springs,
  no holds) unless the entry registers `exact_web::motion::springs`.
  - The engine's code stays in `exact-web`, where its unit tests drive
    holds. Only the registration in `exact-web-capabilities` names it.
- **The use-set.** Motion is:
  - a `transition` that can be a `spring()`;
  - a `swiperight`, height, transform or reorder drag handler;
  - a drag prop.

  CSS plays every other transition.
- **The export group.** `exact_motion` is now `motion_exports!`, which the
  entry invokes only when the plan uses motion.
- **The smoke.** `smoke.mjs web` runs its bare-plan host fixtures, which
  exercise every capability, on a second build with `EXACT_WEB_LINK=all`.
  The app and its tests stay on its production build.
- **Bytes, raw and brotli-11.** Linking motion by use drops about 116 KiB of
  code: the engine, plus the drag code that only the export reached.
  - RealWorld: 1,339,522 → 1,220,278 raw; 391,840 → 359,781 brotli.
  - Video player: 1,167,590 → 1,048,333 raw; 347,265 → 315,748 brotli.
  - Caltrain: 1,227,534 → 1,108,290 raw; 359,245 → 327,940 brotli.

**Stage 3, 2026-09-24: lists.**
- **What counts.** A list the host windows: a `virtualized` one, one with
  `item-height` or `estimated-item-height`, or a handler for a list's edges.
- **Only the entry and the exports changed.** The five list exports are now
  `list_exports!`, which the entry invokes only for such a plan, and
  `Linked.collections` records the group for admission. No code in the list
  engine changed: its host sync is reached only through those exports.
- **Bytes at `b1555c61`, raw and brotli-11:**
  - RealWorld: 1,197,067 → 1,134,237 raw; 342,370 → 326,826 brotli.
  - Video player: 1,025,117 → 962,220 raw; 297,889 → 282,382 brotli.
  - Caltrain: 1,094,390 → 1,031,504 raw; 314,020 → 298,820 brotli.

**Stage 3, 2026-09-24: drag.**
- **A generic seam.** Drags' hooks are generic over the app's data source:
  tracking a handle at create, update and destroy, reconciling it with
  each receipt, and publishing it with each batch.
  - The host holds them as `Option<DragHooks<D>>`.
  - The `host!` macro builds `HostLinks::of(EXACT_LINKED)` for its own
    data type, so the hooks exist only in an artifact that links drag.
  - Tests and native tools boot with `HostLinks::ALL`.
- **The use-set.** Drag is a drag prop or a height, transform or reorder
  handler; each also uses motion, whose export carries drags' input. A swipe
  uses motion alone.
- **Bytes at `ef67ae40`, raw and brotli-11:**
  - RealWorld: 1,126,921 → 1,076,789 raw; 325,880 → 315,533 brotli.
  - Video player: 954,898 → 904,807 raw; 281,444 → 270,640 brotli.
  - Caltrain: 1,024,188 → 974,091 raw; 297,940 → 287,305 brotli.

**Stage 3, 2026-09-24: module replacement.**
- **When a page can take a new data module.** `exact_boot_module` swaps the
  data module of a running page. Two things use it:
  - the dev loop;
  - Rust live replacement in a shipped client whose compat receipt names a
    browser Rust module (LLP 1029.000).

  Web delivery otherwise ships a new dist. The entry writes
  `EXACT_REPLACEMENT` for exactly those two cases. Every other build
  refuses by name, and its replacement path is gone.
- **The Fieldnotes Chrome drive** swaps the module generation, so it runs on
  an `EXACT_WEB_LINK=all` build and skips a shipped one.
- **Bytes at `53f954fc`, raw and brotli-11:**
  - RealWorld: 1,076,789 → 1,064,422 raw; 315,533 → 312,587 brotli.
  - Video player: 904,807 → 878,712 raw; 270,640 → 261,639 brotli.
  - Caltrain keeps Rust replacement and is unchanged.

**Stage 3, 2026-09-24: GPU surfaces.**
- **What counts.** A canvas with a surface: the plan's `surfaces` table has
  a row.
- **The export group.** `exact_surface_record` and `exact_request_active`
  answer only surfaces' records and requests, so they are now
  `surface_exports!`, invoked by the entry for such a plan.
- **Bytes, raw and brotli-11:**
  - RealWorld: 1,064,422 → 1,059,273 raw; 312,587 → 311,049 brotli.
  - Video player: 878,712 → 872,020 raw; 261,639 → 259,253 brotli.
  - Caltrain, whose line map is a surface, is unchanged.
- **The runner's half.** The runner answers an `exactSurface` resource
  itself. It now does so only through `RunnerLinks.surface_answer`, which
  the web host fills from its entry's `Linked`; native hosts and tests pass
  `RunnerLinks::ALL`. A resource that reads a surface's record also counts
  as using surfaces. Bytes at `c798948b`:
  - RealWorld: 966,161 → 952,597 raw; 296,536 → 293,359 brotli.
  - Video player: 780,693 → 767,136 raw; 244,645 → 241,314 brotli.
  - Caltrain uses surfaces: 273,128 → 273,297 brotli.

**D4, 2026-09-24: the router and inspection behind tables.**
- **The router.** The runner holds its router as `Option<Box<dyn Routing>>`,
  built only through `RunnerLinks.router` (`exact_runner::routing`):
  - the VM's verbs and reads;
  - the boot's router slot;
  - reload carry and change publication;
  - the web host's in-place link matching.

  A plan with `routes` uses the router. Native hosts and tests boot with
  `RunnerLinks::ALL`.
- **Inspection.** The agent API's reads go through `HostLinks.inspect`
  (`agent::handle`, and with it the plan digest's `sha2`). The entry links
  inspection by policy in every build, production too (§10, Q3). Unlinking
  it later is a change to the entry alone.
- **Bytes at `e10cbc71`, raw and brotli-11:**
  - Video player: 767,136 → 728,469 raw; 241,314 → 229,994 brotli.
  - RealWorld and Caltrain use the router and pay its dynamic dispatch:
    +953 and +982 raw; +608 and +793 brotli.

**Stage 3, 2026-09-24: Rust modules.** A web entry links the Rust
executor only when the manifest names a `rust.module`
(`contract::web_rust_mode`). An `auto` policy with no module used to link
the browser executor, with nothing to swap in. The compatibility receipt is
unchanged.
- Fieldnotes, LLP, Markdown, Messages, Typetour, the video player and
  Weatherlight drop it.
- Bytes: the video player goes from 728,469 → 704,866 raw and 230,182 →
  223,014 brotli. RealWorld (already `off`) and Caltrain (which has a
  module) are unchanged.

**D9, 2026-09-24: the byte row.** `bun scripts/metrics.mjs --long`, which
the async lane runs per commit, builds RealWorld, the video player and
Caltrain twice:
- as shipped, for `app.wasm`'s raw, gzip and brotli-11 bytes;
- with `EXACT_WEB_NAMES=1`, which makes `build.mjs` keep the name section
  with the same flags otherwise, for their code by capability.

It reports and never blocks. The first run is about 7 minutes with a cold
names target.

**D4, 2026-09-24: the layout engine.** The browser lays out, so the web's
kernel builds no engine tree until a layout is asked for (`3e69aa79`). The
engine's code stayed linked all the same, because every commit named it.
- **The seam.** A commit and a restyle reach Taffy only through
  `exact_kernel::layout::LayoutMirror`: a leaf for a new node, a restyle, a
  child sync, a removal and a dirty mark.
  - `Kernel::new` boxes a `LayoutTree`, as before.
  - `Kernel::on_demand`, the web's, holds none. Until a layout is asked for,
    it commits through `Unmirrored`.
  - Only the layout path boxes a `LayoutTree`.
- **Content regions** check the owner's padding and border with
  `StyleProps::unpadded`, which reads what `to_taffy` would give without
  building the engine's style. A unit test compares the two, `-0` included.
- **Native kernels** make the same engine calls, in the same order.
- **Bytes at `c35f78f7`, raw and brotli-11:**
  - RealWorld: 914,280 → 891,346 raw; 281,900 → 275,418 brotli.
  - Video player: 701,688 → 678,757 raw; 222,573 → 216,368 brotli.
  - Caltrain: 881,207 → 858,277 raw; 273,451 → 267,131 brotli.
- **Absence.** None of the three keeps a function of `taffy` or `slotmap`.
  What remains is the arena's engine-id column: `NodeArena::taffy`, and
  three `Vec` instantiations over `taffy::NodeId`, 140 bytes in all.

**§6, 2026-09-24: generic code.** Generic families in RealWorld at
`e9a38626`, and what removing each whole would save:

| Family | Raw | Brotli-11 |
|---|---|---|
| Hash tables | 20.3 KB | 5.1 KB |
| Drop glue | 20.1 KB | 5.1 KB |
| Collect scaffolding (`from_iter`, `extend`, `fold`) | 36.9 KB | 7.3 KB |
| B-trees (mostly the list engine's) | 54.9 KB | 12.1 KB |

- **Outlined generic code compresses well.** With `-Zshare-generics=no`
  (measured only; it is nightly), LLVM inlines the collect scaffolding:
  RealWorld is 45.9 KB smaller raw but 3.6 KB larger in brotli. So
  iterator chains are not rewritten as loops.
- **SipHash is gone.** Four maps keyed by internal identities
  hash with the kernel's `IdHasher`, and Markdown's code-span index is a
  vector by run length. On the web, `RandomState`'s keys come from fixed
  addresses, so it defended nothing there.
- **`json::object`** was one copy per array length; it now has one body.
- **The settlement pass** no longer clones its effect enum per row.
- **A compact id map was tried and dropped.** It kept dense entries and one
  index over their ids, code shared by every key and value type. It would
  have saved about 2.5–2.9 KB brotli per app (RealWorld 266,870 → 264,020),
  and it matched std in a differential test of 3M random operations. But
  the transaction's per-commit sets start empty and grow on every commit,
  and that growth rewrote the index. On a native churn bench (500 commits,
  each creating 20 rows, moving 220 and destroying 20) it cost 55–70% more
  cycles for 1% more instructions. hashbrown stays; presizing those sets is
  in `QUEUE.md`.
- **Bytes at `e9a38626`, raw and brotli-11:**
  - RealWorld: 898,973 → 890,987 raw; 275,704 → 273,574 brotli.
  - Video player: 685,217 → 682,458 raw; 216,724 → 215,435 brotli.
  - Caltrain: 866,208 → 863,431 raw; 267,479 → 266,532 brotli.

**Deferred: the collections seam.** It waits for the
`llp-ship/20260923-review-followups` run to publish. That run retires the
windowed list (`window.rs`, `heights.rs`) and rewrites
`runner/src/runner/{lists,collection}.rs`, which leaves one list engine to
wrap.
- **The payoff.** In RealWorld, 46.7 KiB of list-engine code never runs,
  about 14 KB brotli. The seam also unblocks about 4.8 KB brotli of the
  kernel's transition and shape-outside parsers. The list engine's own
  `style()` rows, reorder's spring `transition` among them, reach those
  parsers through the shared `bridge::set_style`.
- **The design, with no list-engine file touched:**
  1. `NodeInst.collection` and the region's `ListWindow` become
     `Option<Box<dyn …>>`, over a small trait that `instance.rs` defines and
     implements. Their drop glue then leaves with the engine.
  2. `instance.rs`'s entry points go through a table in `Update`:
     `realize`, `update_all`, `Tree::collections`, `Tree::find`,
     `Tree::wake_collection_edge` and the windowed region's update.
  3. The runner's own `Update` constructions pass `Update::linked(…,
     links)`. `Update::new` keeps every capability, so the list-engine tests
     are unchanged.
  4. `collections_json` and the reorder APIs are guarded by the same table.

### As built: activation, boot and the press path (the activation lane)

**The bench, reproduced at `98d5f996`** (RealWorld served, mobile profile, nine
runs, load 21–27): a tap at `load` reaches the feed in 2,474 ms, shown at
3,401, against React's 212 and 1,417; after a 3-second read, 285 ms against 202.

**2026-09-24, three landings**, nine runs each, alternating, load 33–40, ms:

| | Tap at load → feed | Feed shown | `load` | 3 s read → feed |
|---|---|---|---|---|
| Before (`98d5f996`) | 2,618 | 3,589 | 969 | 322 |
| The download at paint | 1,991 | 3,116 | 1,120 | — |
| Activation at boot | 1,977 | 3,118 | 1,127 | 313 |
| The early GET | 1,900 | 3,046 | 1,134 | 252 |
| React | 277 | 1,632 | 934 | 211 |

- **The download at paint** (LLP 1048.000 D6, as built): the capture script
  starts `app.wasm` at the page's first paint entry. `load` comes later only
  because the glue's modules now share the link with it.
- **Activation at boot** (the same): a served document activates without the
  two animation frames, its data module's realm prepared beside the boot; its
  styles are adopted, not re-set; an early press replays in a microtask.
- **The early GET** (LLP 1027, as built): a data source's GET leaves when the
  source asks for it, and the runner's `request` claims it.

What remains at load: the wasm holds the link for about 2.1 s and lands at about
2.7 s. From instantiation, the boot to the replayed press takes about 350 ms at
4× CPU, `exact_boot` about 220 of it (booting the runner from the checkpoint
about 60%, projecting and hashing the document about 20%); the press then takes
about 250 ms, 160 of them the API.

**The JavaScript pieces (D5), 2026-09-24.** `navigation.js` keeps history,
scroll-follow, links, Markdown rendering, focus and the dev hook. Springs,
holds and drags (`motion-glue.js`: the motion and arrange controllers) and
virtualized collections (`collection-glue.js`: the list engine's browser half,
moved unchanged) are after-paint pieces. Until they arrive, `navigation.js`
stands in:
- A call that needs a piece queues, in order, and starts the load. The queue
  replays when the pieces arrive.
- Before any use, a call that only reconciles (`commit`, `reset`, `destroy`,
  an empty collection commit) is dropped, and a style is set at once, as the
  controller sets one with nothing held.
- A plan that uses motion exports `exact_motion` (stage 3), so its pieces
  start loading at activation. Input readiness waits for any pieces the first
  tree asked for, so no gesture lands before its handler.

RealWorld's boot-path JavaScript falls from 29,342 to 18,596 bytes brotli
(`navigation.js` 14,363 → 3,664), and it never fetches the pieces (12.4 KB).
On the bench at main's `e2ee550a` (nine runs, load 38 → 21), a tap at `load`
sees the feed 39 ms sooner, 2,593 → 2,554 ms, and `load` comes 79 ms sooner.

**The runtime with the document, 2026-09-24.** An idle page's document keeps
the shell's wasm and `navigation.js` preloads in its head, standing in for a
CDN's 103 Early Hints (LLP 1048.000 D3, as built). At `53f954fc` (app.wasm
315,820 bytes brotli; nine runs, load 24 → 16) the feed shows at 2,350 ms, not
2,430, with first paint 239 → 241 ms and content 225 → 222 (15 runs). A fetch
priority for the wasm changed nothing: this bench's throttling shares
bandwidth evenly whatever the priority.

| At `53f954fc`, ms | Tap at load → feed | Feed shown | `load` | 3 s read → feed |
|---|---|---|---|---|
| exact2, as is | 1,461 | 2,430 | 958 | 231 |
| exact2, head preloads | 1,584 | 2,350 | 756 | 230 |
| React | 214 | 1,390 | 884 | 198 |

A tap at `load` reads longer as `load` comes sooner: the feed shown is the
number that moves with the user.

**The boot, 2026-09-24.** At 4× CPU RealWorld's `exact_boot` took about 230 ms.
Five changes cut what it repeated, each measured by a sampled profile of five
traces:
- **A plan is validated once**, where it is made (`Plan::decode`,
  `PlanBuilder::finish`), not again by the runner: about 4 ms.
- **The browser's kernel builds its layout tree on demand**
  (`Kernel::layout_on_demand`): the web never lays out, and commits no longer
  create Taffy nodes and styles. 21 → 3 ms.
- **The adoption digest** is MurmurHash3's x64 128-bit function over the
  plan's bytes, not SHA-256 of their re-encoding (LLP 1048.000 D6, as built):
  8.1 → 0.7 ms. The projection that decides adoption keeps each view's tag,
  props and CSS for the first batch: about 5 ms.
- **The router's boot** skips a third table check and re-validating the value
  it just launched: about 4 ms.
- **Tiny functions are inlined** (`wasm-opt --always-inline-max-function-size
  6`): V8 compiles each wasm function lazily, on the main thread, at its first
  call, and a function of a few bytes costs about 58 µs to compile at 4×.
  The boot compiles 1,123 functions, not 1,314, and app.wasm is 511 bytes
  smaller in brotli. On the bench the feed shows 22 ms sooner.

Lazy compilation is now most of what the boot costs: about 80 ms of
`exact_boot`'s ~143 at `a3de37e0`. With V8's lazy compilation off (a Chrome
flag, measured only), the feed shows 75–100 ms sooner, because streaming
compilation does the work while the bytes download. A page can't choose that.
Wasm compile hints (`metadata.code.compilation_priority`) would let it: in
Chrome 153 they compile hinted functions before their first call, but only
with `--js-flags=--experimental-wasm-compilation-hints`. Without the flag the
section is ignored.

| At `a3de37e0`, ms | Tap at load → feed | Feed shown | `load` | 3 s read → feed |
|---|---|---|---|---|
| exact2 (app.wasm 282,321 B brotli) | 1,411 | 2,219 | 803 | 233 |
| React | 209 | 1,480 | 908 | 203 |

Where a tap at `load` goes (five traces): 1,027 ms waiting for the wasm's
bytes; 9 ms finishing its compilation; 251 ms from instantiation to the
replayed press, `exact_boot` about 143 of it; 210 ms for the press, 150 of them
the API.

**Tags are links, 2026-09-24.** RealWorld's tag is a link to its own rendered
route (`/tag/:tag`, `render=cached`), as React's is, so before the runtime is
up a tap loads that page. At `6b686d1b` (nine runs, load 13–14) a tap at
`load` shows the python feed 356 ms later, at 1,176 ms; the button took
1,334, with the feed at 2,142, and React takes 220, with it at 1,480. After a
3-second read the tap takes 267 ms, not 227: the new route rebuilds the
page's ~150 views.

Two follow-ups, on the same bench (nine runs, loads 13–20):
- **One page.** `/` and `/tag/:tag` are one page (`479b1475`), so a tag
  changes 2 views, not ~150. After a read, the tap takes 249 ms against the
  tag buttons' 251 in the same batch.
- **The download stops.** A link that leaves the page stops the runtime's
  download (LLP 1048.000 D6, `f23f1c6d`). A tap at `load` shows the feed
  261 ms later, not 362, at 1,116 ms; React takes 236, at 1,628.

What's left at load is a document's round trip and its transfer, against
React's API answer.

Round 3, on the same bench (nine runs, loads 8–12):
- **The shell's style is minified** (`2a341b98`). The feed text now needs
  ~2.8 KB of the document, not ~4.0.
- **The feed tabs are links** (`c241d954`).

Results:
- **Tag at `load`:** 249 ms against React's 232, with the feed at 1,083 ms
  against 1,657.
- **After a read:** 223 against 204.
- **Global Feed at `load`, now a link:** 1,313 → 244 ms (React 215).
- **A press that's still a button** (♥, signed out) takes 1,126 ms against
  React's 54.

With React's API over the throttled network (supplementary), its tag at
`load` took 219 ms.

### As built: the core diet (the diet lane)

Each change reads what std read and prints what core printed, bit for bit
and byte for byte, and its differential tests hold it to std. RealWorld's
app.wasm, brotli-11, at each landing (each commit's message has all three
apps and the raw bytes):

| Landing | What left | RealWorld br |
|---|---|---|
| `7033b993` | wasm-opt inlines single callers up to 20, to convergence | −3,019 |
| `e39e1541` | Unicode case tables (`data-` names lowered as ASCII) | −3,763 |
| `113abeaa` | machine paths (source paths remapped) | +40 |
| `02ec4dce` | std's float reader and its 10 KB table (`exact-num`) | −10,873 |
| `7b3759b3` | per-name strings (generated names are one packed string) | −1,063 |
| `6d60a438` | the engine's small `BTreeMap`s and sets (sorted vectors, bitsets) | −9,566 |
| `918da815` | two sorts (touched keys, detached slots) | −580 |
| `c798948b` | `route::Params`' `BTreeMap` | −2,872 |
| `d3b77fa8` | serde_json from the JS bridge (`exact_js_value::json`) | −11,676 |
| `c35f78f7` | six small hash tables | −1,321 |
| web: the browser's kernel links no text measurer | the monospace measurer | −462 |
| num: one float printer everywhere | core's Grisu, Dragon and bignum | −6,365 |
| plan: a decoded plan's rows read through a sticky reader | `Plan::decode`'s per-field error copies (18.2 → 8.7 KB raw) | −535 |
| kernel: generated style code converts once per codec, names by one table | two masked copies, per-row conversions, 26 string matches | −1,101 |
| `bde196a1`–`6aa8cc15` | per-row finiteness copies, two sort instantiations, the store's generic constructor, joined boot-path text | −2,614 |
| `629bb030` | Unicode case tables again (a merge lowered a failure's kind with `to_lowercase`; ASCII now) | −3,455 |
| num: `text!` fills `{}` holes without `core::fmt` | `core::fmt` from boot, adoption and the press: the batch, CSS, the journal's lines, keys, the realm's call key, JSON numbers, the digests; a reply's missing value is made only when read | +447 (the split's primary −1,964) |

The float printer needed list-engine lines, approved for exactly those:
`window.rs`'s four `f64::clamp` calls (std's assertion message prints the
bounds as floats) and two printers in `collection/`. `Plan::encode` (3 KB)
is reached only from inspection's plan digest (`Runner::inspection_digest`
through `agent::handle`), so it is already linked by use: an artifact
without inspection drops it.

Sized at `629bb030` against a fresh profile (boot, adoption, the tag press
as a runtime press; 1,355 of 2,926 functions run), the split's primary is
169,996 B (the carried profile had 170,973) and the whole module 263,090 B.
What is left, in brotli-11 KB, each family's code or bytes removed (all
data stays in the primary: wasm-split moves functions only):

| Family | Whole | Primary | What removing it takes |
|---|---|---|---|
| data only deferred code reads (30.8 KB raw) | — | 11.1 | the split moves data too: exact symbol bounds from the linker, a post-split pass (Charlie's, with the split) |
| panic locations and paths | 10.2 | 8.0 (locations 4.0) | nightly `-Cpanic=immediate-abort -Zlocation-detail=none`; the web prints neither (`panic_output()` is `None` on wasm32-unknown-unknown) (Charlie's) |
| plan validation at load | 4.3 | 4.0 | Charlie's |
| derived `Debug` (code, names, escape tables) | 8.9 | 3.8, nearly all data | left: 14 of its 42 sites put `{e:?}` in a batch's error text; its data leaves with the data split |
| hash maps, `BTreeMap<u32, Value>` | 12.2 | 3.9 | declined (below) |
| `core::fmt` on the boot path | — | 1.6 | landed (above): 2.0 KB left the primary. `snapshots_json` never pinned it: its writes run only for a collection, and RealWorld's boot has none |
| dlmalloc | 2.1 | 2.1 | left: another allocator changes memory behaviour |
| JSON trees on the reply path | 1.4 | 1.6 (about 1 net) | left: under the 2 KB bar |
| Unicode tables (markdown punctuation, `Debug` escapes) | — | 3.2 | leave with the data split |
| the app's plan and JS module | — | 7.3 | the app's own bytes, needed before the press |
| the split's import and export tables | — | 2.8 | the split's design |

Maps were sized and declined: one non-generic map would touch about 40
sites across kernel, runner and web for 2–3 KB net, carries the
commit-path risk that sank the compact id map (+55–70% cycles), and cannot
move the goal. Feed shown by the activation lane's rule (1,700 ms at
173.6 KB, 5 ms per KB, 6 ms without validation): the stable split 170.0 KB,
1,682 ms; nightly flags 162.0 KB, 1,642 ms; and no validation 158.0 KB,
1,616 ms; the data split on top 149.5 KB, 1,573 ms. Parity (~1,470 ms)
needs 129–133 KB, which nothing sized here reaches. With `core::fmt` off
the boot path the stable split's primary is 168.0 KB (1,672 ms). The rule
models a runtime press at load; since tags became links (`98958c9e`), a
tag tapped at load loads its rendered page instead.
