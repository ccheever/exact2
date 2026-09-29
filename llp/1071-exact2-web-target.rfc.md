# LLP 1071: exact2's web target — a small JS runtime, Contract compiled to JavaScript, the DOM as the tree

**Type:** RFC
**Status:** Draft, with rulings (§8): Charlie approved the spike on 2026-09-28, then ruled the same day that this is exact2's web support, a new compile target for the same Contract — not a new framework, not "Exact 3" — used by default where an app qualifies, the wasm target the fallback until §7's gaps close.
**Systems:**
- Contract: a second backend, from the validated plan to JavaScript (`host/web-js`, crate `exact-web-js`).
- Web host: a JavaScript runtime beside `exact-web` (`host/web-js/rt.js`); the glue, capture and adoption.
- Build: `host/web/build.mjs` builds the JS target by default and falls back to wasm with the refusal named (`--js`/`--wasm` force one); a build-time stylesheet from `css.rs`. The dev loop, the agent's web host, the smoke and metrics run what it makes (§7, "The tools").
- Render server (LLP 1048): unchanged as the producer of documents; the checkpoint gains instance state for D6.
- Agent (LLP 1012): the nine operations over the JavaScript runtime, in a development module.
- Native hosts: none.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5), from 2026-09-28: the spike (§7, branch `exact3-web-spike`), then on main as `host/web-js`.
**Date:** 2026-09-28
**Related:**
- LLP 1047 (pay for what you use): §1 and §10's measurements, D9's budget, §8's "compile the plan instead of interpreting it" (`1047:440-445`), the DOM-only kernel (`1047:1063-1071`).
- LLP 1047.000 (staged loading): runtime-up timings (`1047.000:36-55`), levers sized and declined (`1047.000:99-111`).
- LLP 1048, 1048.000, 1048.001: documents, the checkpoint, adoption, activation and the capture script.
- LLP 1027 D6: TypeScript sources on the web, the browser as executor (`1027:688-760`).
- LLP 1004 D4 (the roster), 1005 (the plan and the runner), 1007 (the web host), 1012 (the agent), 1038 (the router), 1002 D6 (motion delegated to CSS).
- `rules/RULES.md` §Budgets and §Scope; `rules/DEFERRED.md` §Authoring models.

## Summary

Charlie, 2026-09-28: *"I'm interested in making the best possible version of
Exact for the web. The biggest problem is a fairly big runtime wasm, ~300 KB,
that has to be downloaded before you can really do anything. I'd love to see
that get down to like 20 KB. If we reimagine this from the beginning, even an
Exact 3, could we get that down to ~20 KB? What sacrifices?"*

The answer is yes on the web, and not by trimming.

- **Trimming exact2 stops at about 150 KB brotli** (§2, an estimate). What is
  left is a Rust interpreter, a tree mirror, a batch protocol and Rust's std.
  In a browser, each duplicates something the browser already has.
- **About 20 KB needs a different web runtime** (§3):
  1. Contract compiled ahead of time to JavaScript: the code is the plan.
  2. The DOM is the tree; fine-grained signals write to nodes.
  3. Static styles go to a build-time stylesheet; only dynamic values are inline.
  4. A JavaScript runtime of about 5–8 KB.
  5. Everything else loads on first use.
  6. Optionally, resumability: HTML plus a ~2 KB loader.
- **The estimate:** about 6–10 KB of runtime plus generated code at 1–2× the
  plan's brotli size. That is ~10–15 KB for a small app and ~20–25 KB for
  Caltrain, against 242 KB and 302 KB of wasm today.
- **What it costs** (§5): one engine on every host becomes two runners held
  equal by a conformance suite (§4). Plans stop being validated data on the
  web. Rust sources on the web load as their own wasm. Formatting needs specs.
  Inspection leaves the shipped bundle.
- **What stays** (§6): Contract, the plan, the native hosts, the renderer, the
  data seam, one source.

It is the rule "the web is the standard" taken one step further: on the web,
stop emulating the browser and be it. It is a compile target, not a framework:
the same Contract, the same plan and the same data seam, lowered for the web
as the Apple and Linux hosts lower them for theirs.

## 1. Measured

2026-09-28, at `2cf49c21`, on an Apple-silicon Mac.
- **Build:** `host/web/build.mjs` on the web toolchain (`WEB_TOOLCHAIN`, LLP
  1047 §10). These builds are unsplit: binaryen 133 is installed against the
  pinned 132, so the ~8–11 KB inspection stage is inside `app.wasm`.
- **Compression:** brotli quality 11.
- **Attribution:** a names build, each function to the first exact2 module its
  demangled name mentions (LLP 1047 §1's rule). Each group is brotli'd alone,
  so the groups sum to ~260 KB against the whole's 242: shares, not a ledger.
- **Comparable runtimes** are from knowledge, not measured here, except
  React's counter (`1047:66`).

### Sizes

| Artifact, brotli-11 | Bytes |
|---|---|
| Video player `app.wasm` (35 lines of Contract) | 242,316 |
| Caltrain `app.wasm` | 302,148 |
| `glue.js` | 16,949 |
| `navigation.js` | 6,954 |
| document, input and timer glue | ~4,900 |

The video player's wasm went 368 KiB (`1047:64-67`) → 365,025 B
(`1047:103-120`) → 242,316 B. Linking by use, the core diet and the web
toolchain took a third; the rest is the core.

### What loads when (Caltrain)

- **First paint** is the served HTML, 5.7 KB, with no wasm (LLP 1048.000).
- **First interaction** needs ~330 KB: the wasm, the glue and the data module.
- **Activation** runs when the page is idle after `load`, or at the first
  input, whichever comes first; a press before it replays once
  (`1048.000:247-250`, `1048.000:408-414`).
- **Runtime up** is ~1,890 ms on the bench's mobile profile, against ~950 ms
  for React SSR to hydrate (`1047.000:36-55`).

### Where the video player's bytes go

| Group | KB brotli |
|---|---|
| Runner (boot, actions, events, commit) | 35.7 |
| Kernel tree, transactions, ids, arena | 30.0 |
| Kernel styles (generated `StyleProps`, style, SVG) | 26.2 |
| Web host (batch, CSS, ABI) | 24.6 |
| Plan decoder and validator 12.1, values 12.8 | 24.9 |
| Rust std, hashbrown, glue | 24.4 |
| Instances and reactivity | 13.9 |
| VM and stdlib | 10.1 |
| Served-page adoption | 9.1 |
| Motion | 9.1 |
| Inspection | 9.1 |
| Data seam | 6.8 |
| `exact_num` 5.2, libm 4.1, dlmalloc 2.3, `core::fmt` 2.3 | 13.9 |
| Data segments | 21.8 |

Taffy's layout code is not linked on the web. The largest functions
(`twiggy top`, raw): `vm::eval` 16.5 KB, `StyleProps::set_dynamic` 14.7,
`txn::apply_document` 13.2, `Plan::decode_from` 9.6, `Plan::validate` 7.8.

### What the web host does today

- **The browser lays out.** Real CSS is emitted from style rows
  (`host/web/src/lib.rs:7-13`, `host/web/src/css.rs:1-13`). The kernel never
  lays out on the web: `Kernel::on_demand` starts with `layout: None`
  (`kernel/src/kernel.rs:250-263`).
- **The browser measures text** (`host/web/src/host.rs:281-295`).
- **The DOM is built from JSON batches** (`host/web/src/batch.rs:1-5`), parsed
  and applied by the glue (`host/web/glue.js:1323`, `606-621`).
- **So the wasm keeps a second tree.** It holds a full kernel mirror of the
  DOM, and style rows it parses and then prints back out as CSS text.

### Levers already sized and declined

| Lever | Yield, KB brotli | Where |
|---|---|---|
| A DOM-only web kernel | 10–15 | `1047:1063-1071` |
| Errors as codes | 5–8 | `1047.000:101` |
| A non-generic runner | 0 | `1047.000:106` |
| Non-generic maps | 2–3 | `1047.000:110` |

Parity with React on RealWorld's feed timing needed 129–133 KB, which nothing
sized reached (`1047:1019-1020`). RULES has no byte budget: D9's "core web
payload" row (`1047:333-343`) became a reported outcome, never blocking
(`1047:478-488`).

### Comparable runtimes (knowledge, min+brotli, approximate)

| Runtime | KB |
|---|---|
| Qwik loader | ~1 |
| Svelte 5 | ~3–10, plus compiled code |
| Preact | ~4–5 |
| Solid | ~7 |
| htmx | ~15 |
| React + ReactDOM | ~45–55 (a Vite counter measured 58 KiB, `1047:66`) |
| wasm3-class interpreters | 60–100 |

## 2. Why trimming stops at about 150 KB

Every sized lever, applied to the video player (estimates, from 242 KB):

| Lever | KB |
|---|---|
| Inspection out of the shipped bundle (the stage, once binaryen matches) | −9 |
| Motion linked by use (the player has no transition) | −9 |
| Adoption only on served pages (the player isn't served) | −9 |
| A DOM-only kernel | −10 to −15 |
| CSS as text at build, instead of `StyleProps` at runtime | −10 to −15 |
| Errors as codes | −5 to −8 |
| Staging what the first interaction doesn't run | −10 to −20 |
| Data segments those take with them | ~−5 |
| **Left** | **~150–170** |

Add ~24 KB of glue and the first interaction still costs ~175 KB. Caltrain
stays ~60 KB above that, for its plan, adoption and capabilities.

What remains is five things a browser already has:

| What the wasm carries | KB | What the browser already has |
|---|---|---|
| An interpreter: VM, plan decoder and validator, values | ~35 | a JavaScript engine that runs code |
| A tree and its transactions | ~20–30 | the DOM |
| A style model printed as CSS | ~10–26 | the CSS parser |
| A boundary: batches, ABI, JSON, and the glue across it | ~25 + 17 JS | direct DOM calls |
| A language runtime: std, hashbrown, dlmalloc, libm, `fmt`, `exact_num` | ~38 | `Map`, the GC, `Math`, `String(n)` |

Wasm can't touch the DOM, so the boundary is structural: every mutation is
encoded in Rust and decoded in JavaScript. A plan is data, so it needs an
interpreter. The floor is the Rust/wasm runtime itself. LLP 1047 §8 named
the way past it and set it aside: compile the plan instead of interpreting
it (`1047:440-445`).

That note priced compiling at three things. Here, on the web only:
- **The 20 ms plan restart.** Emitting JavaScript from a plan is a
  build-time pass of milliseconds. The spike measures it (§7).
- **Plans delivered without a new binary.** A web release is static files
  either way (§5.2).
- **One runner shared by every host.** This is the real cost (§5.1).

## 3. The design

The web gets its own runner, written in JavaScript and fed by a compiler
backend. The plan stays the interface: the backend reads the plan the Rust
runner reads, after `Plan::validate`, so both runners start from one artifact.

### D1 — The plan compiled ahead of time to JavaScript

- **Input:** the validated plan, not Contract's AST. The plan is already the
  interface between compiler and runners, and it is what the conformance
  fixtures (§4) are.
- **Output:** ES modules, one per route chunk, and a manifest.
  - **A component** becomes a function. Its static structure is a
    `<template>`, cloned. Its bindings are effects on the cloned nodes.
  - **State slots** become signals, `derive`s become computeds, actions become
    functions that write signals.
  - **Expressions** become JavaScript expressions. Roster calls become imports
    from the runtime's roster, so the bundler drops unused entries.
  - **Binding sites keep their plan node index** as a `data-s` attribute where
    a node is dynamic, so the Rust renderer's document and the generated code
    name nodes alike (D6).
- **Gone:** the decoder, the validator and the VM. Validation runs once, at
  build.
- **Tree shaking returns to the bundler.** LLP 1047 had to make the compiler
  the tree shaker because a plan is data (`1047:39-44`). Generated code
  imports what it uses; Rolldown drops the rest.

### D2 — The DOM is the tree

- **No mirror.** There is no kernel, no `ViewId` map and no batch. An effect
  writes one property, attribute or text node.
- **Commits.** An action's writes land together: effects run after the action
  returns, flushed once per event turn, as the Rust runner commits once per
  event.
- **`when`** is a region between two comment anchors; its subtree and its
  effects are disposed when it closes.
- **Keyed instances** (`for … key`) reconcile by key over DOM nodes: moves by
  longest increasing subsequence, instance scopes disposed on removal. Keyed
  row state lives in keyed data, as LLP 1068 §5.3 already requires.
- **Regions** (`runner/src/instance/region.rs`) use the same anchors.
- **Events** are delegated: one listener per event type on the root,
  dispatched by a `data-on` attribute to the handler the component registered.
  The capture script's `data-exact-on` (`host/web/capture.js`) is the same
  mechanism before activation.

### D3 — Styles to a build-time stylesheet

- **Static rows** become classes in one stylesheet emitted at build. The CSS
  comes from `css.rs`, run at build instead of in the browser, so the
  row-to-CSS projection stays one Rust implementation.
- **Dynamic rows** become `style.setProperty(name, value)`. The backend emits
  each bound row's unit and name rule inline, so only rows the app binds cost
  bytes.
- **Motion** is already CSS on the web (LLP 1002 D6). A static `spring()`
  becomes keyframes at build; a dynamic one loads the spring module (D5).

### D4 — A runtime of about 5–8 KB

| Piece | ~KB brotli |
|---|---|
| Signals, computeds, effects, disposal | 1.0 |
| Keyed reconciliation, regions | 1.0 |
| Event delegation | 0.5 |
| Data-seam client: resources, requests as values, grants, `conforms` | 2.0 |
| Router: locations, History, link interception | 1.0–1.5 |
| Timers under a seekable clock | 0.5 |
| Roster core | 0.5–1.0 |

**Resources and the data seam.** A `resource` compiles to a keyed cell of
pending, answer or failure, keyed by source and arguments. The seam keeps
LLP 1027's contract: `answer`, `parse`, requests as values the host runs
under grants, the store. The TypeScript module still runs in its private
realm (`1027:688-760`). The `exact_js.call` import and its JSON copies into
wasm memory go away: the runtime calls the module directly. Shapes are
checked at the seam, as `Value::conforms` does today.

**The router** is LLP 1038's table compiled to patterns and lazy chunks.
Navigation data follows LLP 1048.001 D7. Much of `navigation.js` becomes this
module.

### D5 — Everything else on first use

Generated code `import()`s a capability where it is first used. This is LLP
1047 D1's third tier, with the bundler finding the use-set:
- springs and dynamic motion; exit and layout transitions;
- text flow (already its own wasm); Markdown reading; the editor (already
  separate);
- SVG and Canvas 2D beyond the elements themselves; the GPU module and
  storage (already separate);
- virtualized collections, drag and reorder;
- `formatDate` and `formatNumber` (`runner/src/format.rs`, already linked by
  use);
- inspection and the agent's nine operations: development builds only (§5.5).

A plan that uses a capability the JavaScript runtime lacks is refused at
build, by name, as LLP 1047 D6 refuses an unlinked one. That lets the runtime
grow one capability at a time.

As landed, the backend and `host/web-js/build.mjs` refuse by name: a
dynamic `virtualized`, `scrollIntoView`, a Canvas 2D surface drawn by a
TypeScript source, a TypeScript and a Rust source in one app, native
modules, the events and dynamic rows in §7's table. `host/web/build.mjs`
prints the refusal and builds the wasm target. RealWorld, the video player,
completion-storm, Motion Gallery, Typetour and Carousel build JS, and so does
Bluesky, outside the repo (`EXACT_APP_DIR`, §7 below); Caltrain and
Weatherlight (their GPU modules, which only the wasm build makes yet),
Canvas Gallery (its TypeScript draws) and Update Lab build wasm. Given the wasm
build's GPU module (`--plan`, as conformance builds it), Caltrain builds JS
and is equal on every step.

### D6 — Documents, activation and resumability

**LLP 1048 is unchanged as the producer.** The Rust renderer, running the Rust
runner, still makes every document, at build or per request. First pixels
stay HTML, so "App JS executed before first pixel: none" holds.

**Activation, first form (the spike's).** An undeclared route is `eager`:
the served head carries `<link rel=modulepreload>` for the entry and any
chunk it imports statically, so the ~20 KB runtime downloads while the
document streams; the capture script imports it at the page's first paint
entry, once the document is parsed, so no module script runs before first
pixel. `activate=idle` keeps 1048.000's policy (idle after `load` and first
paint, `1048.000:247-250`); `activate=interaction` fetches nothing before the
first press or edit. On every policy a press before activation starts the
import and replays once. The runtime runs the route's components against the
checkpoint, claiming existing nodes by `data-s` instead of creating them. A
mismatch renders fresh once, as today. The difference is ~20 KB to fetch
instead of ~330.

**Early flush.** The render server sends a page's head as a browser's
navigation (`Sec-Fetch-Dest: document`) arrives, before any data is asked:
doctype, charset, the capture script, the entry's `modulepreload`s and the
stylesheet, after a 103 Early Hints naming the same preloads. The title,
metas, document and checkpoint follow when the render settles, as one
chunked, brotli-flushed response (`host/render/src/stream.rs`). The status
goes with the head, so a flushed page is a `200`, `private, no-cache`, with
no ETag. A render that then turns 404, 410 or 503 sends that document; one
that fails sends the 500's text after the head; only a flushed 200 is kept
at the origin. Crawlers, `curl`, CDNs, conditional requests and `HEAD` send
no `Sec-Fetch-Dest`, so they still wait and get the real status and
validators. A flushed page needs `lang`/`dir` that the plan alone decides. The
JS render path (`render.mjs --serve`) streams the same way. Chrome acts on a
103 only over HTTP/2, so on the bench's HTTP/1.1 loopback the 103 changes
nothing; it is there for a CDN in front.

**A smaller page.** The checkpoint's answers are JSON text (lossless:
records `{"r":[…]}`, `some` `{"s":…}`, `none` `{}`; `-0` and non-finite
numbers kept), not base64 value bytes, so brotli matches them against the
document's own strings. The Rust document over the JS shell carries the
stylesheet's class where an inline style equals one, and drops the wasm
runtime's view ids (a link keeps an empty `data-view`, as the runtime's
links have). RealWorld `/`: 9,652 → 6,388 B brotli as sent, pixel-identical
with and without JavaScript. Adoption is unchanged: RealWorld 21/21 served
(`--urls`) and client-rendered, Weatherlight 12/12, the synthetic plans
green.

**On the RealWorld bench** (mobile profile, 25 runs interleaved, medians
with p25/p75, same-session React and Octane SSR controls;
realworld-bench `bench/results/flush20.md`, `flush170.md`; exact3-web-spike
`1877567a`, `59099fbf`), rendering every request as the SSR servers do:

| API per answer | FCP (JS / React SSR / Octane SSR) | runtime up, no input | page-2 press → answered | code KB before interactive |
|---|---|---|---|---|
| 20 ms | 388 (316–396) / 344 (312–392) / 372 (328–404) | 659 / 950 / 1,091 | 453 / 407 / 390 | 26.1 / 81.0 / 103.8 |
| 170 ms | 372 (328–404) / 360 (312–384) / 372 (308–400) | 645 / 948 / 1,038 | 307 / 324 / 283 | 26.1 / 81.0 / 103.8 |

FCP stays within noise for all three at both API speeds. The emulated 150 ms
RTT and 1.6 Mbps link set it, and the flush keeps a 170 ms backend out of it.
The runtime is up about 300 ms before React SSR hydrates, on a third of
React's code. The page-2 press is within noise.

**Every served page becomes a conformance fixture.** Adoption succeeds only if
the JavaScript runner's canonical document equals the Rust renderer's.

**Resumability, the second form (optional).** The document carries its
checkpoint plus each instance's slot values, and a ~2 KB loader: the capture
script, delegation and a chunk map. The first event on a `data-on` element
imports that component's chunk and the runtime, restores its signals from the
checkpoint, and binds effects to the existing nodes. Nothing re-renders.
- **It needs:** the checkpoint (`1048.000` D6) gains instance state. Today it
  deliberately carries answers, never a store (LLP 1048 D4).
- **It gives:** LLP 1048.001's `interaction` policy nearly for free, and a
  page that never needs its runtime loads none (already D5 `never`).

### How it meets LLP 1047's capabilities

| 1047 capability (`1047:348-366`) | Here |
|---|---|
| Motion | CSS as today; springs lazy |
| Markdown markup, editor | lazy; the editor is already a separate wasm |
| Text flow | lazy, the existing wasm |
| Router | a runtime piece, ~1–1.5 KB |
| Virtualized collections, drag | lazy |
| Rust data modules | their own wasm, lazy (§5.3) |
| Fonts, media | the elements' own attributes; media glue lazy |
| Inspection | development builds only |
| GPU canvas, storage | unchanged, already loaded on demand |

## 4. Parity and conformance

Today the web and native agree by construction: one runner. Here they agree
by test.

**The fixtures:**
- `contract/corpus/*.contract` (40 programs);
- each app's smoke, as `scripts/smoke.mjs` drives it;
- the roster's oracle tables (`runner/tests/it/format.rs`, `runner/src/stdlib.rs`'s tests);
- every rendered route's document (D6).

**The harness:** one agent script (`tree`, `tap`, `type`, `clock`, `state`,
`logs`) run by `scripts/agent.mjs` against the Rust runner on the headless
Linux host and against the JavaScript runner in headless Chrome, with the
outputs compared. The clock is in the script, so timers and settlement
compare exactly. It runs per commit on the async lane, not as a sixth
blocking check.

**Byte-identical** (a difference is a bug in one runner):
- slot values after every operation, in canonical value bytes (LLP 1005 §3);
- the requests sent across the seam: source, arguments, method, URL, headers;
- the roster's outputs over its fixtures;
- the canonical document of every route (LLP 1048 D1's digest, ids excluded);
- the router's locations and URLs;
- the order of events, action effects and timer firings under the seekable
  clock.

**Spec-equivalent** (held to a written rule, compared by effect):
- CSS: classes and inline styles instead of per-node declarations, compared
  by computed style in Chrome;
- the moment effects reach the DOM within a turn;
- error and log wording: codes in the JavaScript runtime;
- node identity in `tree`: plan sites and keys, not the Rust runner's counter
  ids (`runner/src/instance.rs:219-222`);
- performance and memory.

**Cheaper than it sounds.** Values are already JavaScript's: numbers are
binary64 (`plan/src/value.rs:17-18`), `length` counts UTF-16 code units
(`runner/src/stdlib.rs:130-134`), numbers print at JavaScript's boundaries
(`runner/src/stdlib.rs:252-253`), and formatting is `Intl`'s `en-US`
(`runner/src/format.rs:10-11`). The Rust roster was built to the browser's
oracle; the JavaScript one calls the oracle.

## 5. What it gives up

1. **One engine on every host.** Native keeps the Rust runner; the web gets a
   JavaScript runner. They are held equal by §4, not by construction. Every
   runner feature is built twice. I am ~90% confident this is unavoidable at
   20 KB: any shared engine is either an interpreter (wasm3-class, 60–100 KB,
   before a runner) or Rust in wasm (§2's floor).
2. **Plans as validated untrusted data on the web.** The web ships code. A
   plan is validated at build, and the web artifact is JavaScript like any
   site's. Web OTA becomes ordinary JavaScript delivery. Native delivery (LLP
   1030) is unchanged.
3. **Rust data sources cost bytes on the web.** A Rust module (Caltrain's
   `caltrain-logic`, `apps/caltrain/app.json:22`) ships as its own wasm, with
   no runner or kernel, loaded on first need. When the checkpoint holds its
   answers, that need comes after first interaction. TypeScript is the
   zero-cost default there, as LLP 1027 made it the default for logic.
4. **Byte-identical formatting by construction.** Every roster function needs
   a written spec and shared fixtures (§4). `Intl` can differ across browser
   versions where the Rust tables don't.
5. **Inspection in the shipped bundle.** The agent's operations are a
   development module. The smoked artifact is the shipped artifact plus that
   module, which is LLP 1047 §9 Q3's trade answered the other way.

Also given up:
- **The dev loop's single runner.** The web loop runs the JavaScript runner,
  so a native-only bug is found on the async lane, not in the seconds loop.
- **The letter of DEFERRED §Authoring models.** "Nothing runs JavaScript above
  [the seam] — not in Contract, not in the tree." Authors still write no
  JavaScript, but the tree is now driven by generated JavaScript. That is a
  DEFERRED trade (§8).

## 6. What stays

- **The native hosts,** Apple and Linux, with the Rust runner, kernel and
  Taffy: unchanged.
- **Contract,** the language and the compiler through the plan: unchanged.
- **One source.** An app is one `app.contract` and its data modules, for four
  surfaces.
- **The plan** as the one interface, and the conformance currency.
- **The renderer** (LLP 1048): documents, checkpoints, caching, the capture
  script.
- **The data seam** (LLP 1016, 1027): sources, requests as values, grants, the
  store, the TypeScript module in its realm.
- **The agent's nine operations,** on every host.
- **CSS as the standard.** `css.rs` still decides every row's CSS, at build.

## 7. The spike

Branch `exact3-web-spike`. It measured the claim; after §8's ruling it landed on main as `host/web-js`.

1. **The backend.** `contract` gains a JavaScript emitter over the validated
   plan (D1), enough for the video player: components, slots, actions,
   `when`, text and attribute bindings, `input`, `video`, `focus`.
2. **The runtime** (D2–D4): signals, delegation, regions, timers, the roster
   pieces the player calls.
3. **The stylesheet** (D3) from `css.rs` at build.
4. **The video player,** run by `scripts/agent.mjs web` with its smoke.
   Measure bytes (brotli-11, runtime and generated code apart) and
   time-to-interactive on the bench's mobile profile (150 ms RTT, 1.6 Mbps,
   4× CPU).
5. **Caltrain:** keyed lists, the router, resources, search, theming,
   adoption of its served pages (D6, first form). Its Rust logic runs two
   ways, measured apart: as its own lazily loaded wasm, and as LLP 1027's
   byte-identical TypeScript port (`1027:1599-1848`).
6. **Conformance:** both apps' smokes, and the corpus programs they touch,
   compared across the two runners (§4).

**What would confirm the estimate:** the video player ≤ 15 KB brotli for
first interaction, Caltrain ≤ 25 KB before its data module, and runtime-up at
or under React SSR's ~950 ms.

**What would stop it:** generated code above 3× the plan's brotli size, a
runtime above 12 KB, or a Caltrain semantic that can't be matched without
reimplementing the kernel. Any failing measurement gets three rounds, then a
report.

### Spike results

Branch `exact3-web-spike`, 2026-09-28. Numbers are measured on an
Apple-silicon Mac with headless Chrome 154 unless marked *estimate*. Sizes are
brotli-11 unless marked.

**Decision recorded (Charlie, 2026-09-28):** pre-rendering "keep both
options available to allow for different deployment scenarios, but Rust
should be the primary/default." The build takes `--render rust|js`, default
`rust`: (a) the Rust render host (`exact_render`) writes the page over the
JavaScript runtime's shell and the runtime adopts it; (b) the generated
JavaScript renders the same page under Bun (`host/web-js/render.mjs`) for JS
edge runtimes and TypeScript-heavy deployments.

**What was built.**
- `exact-web-js js <app.contract | baked app.plan>`: the plan's bytecode to
  JavaScript (structured forward jumps as labeled blocks), the view as DOM
  construction, static rows to a class stylesheet computed by the web
  host's own `tag_for`/`props_for`/`css_text`/`host_css` (a new
  `exact_web::host::template::parts`), dynamic style units read from
  `css_text` itself.
- `host/web-js/rt.js`: signals, commits with rollback on refusal, a
  settlement pass, typed writes, resources with tickets and LLP
  1054.000.000's kept requests, mutations (`send`, `pending`, declared
  refreshes, `then`), the durable store, row slots, `when`/`match`/keyed
  `each`, timers on the driver's clock, placeholders, the router
  (`route/src` ported; the web host's `navigation.js` reused for history),
  adoption from a checkpoint, capture and replay, Markdown as a loaded
  capability (the web host's pieces in a 24 KB wasm, `renderMarkup`
  reused).
- Data: TypeScript `app.ts` bundled into the page (RealWorld); Rust sources
  through their logic module over ABI 3 after first paint (Caltrain's own
  module, or one generated from the DataSource an app's web build bakes).
- `host/web-js/conform.mjs`: the conformance harness (§4 below), and
  `host/web-js/bench.mjs` / `render-bench.mjs` for the numbers here.

**Conformance.** The same plan through the Rust web runner and the
JavaScript runner in one Chrome, driven by the same `scripts/agent.mjs`
operations; after each step the typed state, the tree, layout boxes by
testId and a screenshot are compared; `app.test.contract` files run on both.
- RealWorld (a scripted scenario against the hosted API: an article with
  Markdown, history back, a tag, the global feed, a failed sign-in, the
  auth pages): 16 steps compared, all equal after the fixes the harness
  found.
- Caltrain: state, tree and layout equal (158 testIds within 0.01 px);
  screenshots differ only where the GPU surfaces are (not built yet);
  `app.test.contract` 3/3 on both. Video player and completion-storm:
  every step equal. Four synthetic plans (regions and keyed reorders, row
  slots, dynamic styles, timers): every step equal.
- Adoption: a page from either renderer, adopted, against a fresh
  JavaScript render: every step equal (RealWorld).
- Other in-repo apps are refused at build by named features not yet
  compiled (native modules, dynamic SVG paint and clip-path, keyframe
  animations, virtualized lists, `reachstart`, dynamic `line-height`):
  the harness lists each (see the full run in the spike report).

**RealWorld, the headline** (`bench.mjs`, LLP 1047.000 §1's method: mobile
profile, 150 ms RTT, 1.6 Mbps, 4× CPU, cold profile per run, 5 runs,
medians; pages served per request against api.realworld.show, `cached`
routes kept at the origin; the press is the sign-in form's submit on
`/login`, since signed out every press on `/` is a link):

| ms / bytes | exact2 served (wasm) | JS, Rust-rendered (a) | JS, JS-rendered (b) | JS, client only |
|---|---|---|---|---|
| FCP, `/` | 400 | 420 | 316 | 752 |
| Content painted, `/` | 400 | 420 | 316 | 1,201 |
| Tag tapped at `load` → feed | 1,140 | 1,111 | 879 | (no tag on screen at `load`) |
| Runtime up, after a press at `load` | 2,329 | 652 | 661 | 678 |
| A press that needs the runtime, at `load` → effect | 3,084 | 1,440 | 1,431 | 1,469 |
| Origin bytes before content | 6,979 | 7,892 | 7,032 | 22,092 |
| Origin bytes before the press answered | 340,164 | 24,413 | 23,762 | 22,092 |

The React SPA and SSR columns wait for the React bench, which isn't on this
machine; `bench.mjs --config` takes their selectors. LLP 1047.000 §1's React
SSR figure for runtime-up was ~950 ms on its own machine and network.

**Bytes before interactive, growth by step** (fresh page, JS target):

| brotli B | spike | + semantics, router, adoption (steps 1–3) |
|---|---|---|
| Video player: runtime share + generated code | 2,099 + ~940 | 3,738 + ~1,030 |
| Video player: page + `app.js` | 4,273 | 6,006 |
| Caltrain: runtime share | 3,295 | 7,315 |
| Caltrain: `app.js` (runtime, generated, router, `navigation.js`) | 7,367 | 13,239 |
| Caltrain: pre-rendered page | — (client-rendered) | 6,054 (a) / 5,292 (b) |
| RealWorld: `app.js` (with `app.ts` and the router) | — | 19,152 |

**Time to interactive, the same method as before** (a press retried until it
answers; cold; unthrottled / 4× CPU with 150 ms and 1.6 Mbps):

| ms | spike | now | exact2 wasm |
|---|---|---|---|
| Video player | 69 / 610 | 59 / 583 | 102 / 2,022 |
| Caltrain | 89 / 602 (FCP 524) | 71 / 640 (FCP 300, pre-rendered) | 267 / 2,728 (FCP 384) |

**The two renderers** (`render-bench.mjs`: every request renders,
`Cache-Control: no-store`; `rust` is the native render host, one render
worker for Caltrain, four for RealWorld; `js` is one Bun process; after the
render-host fixes below):

| | Caltrain `/` rust | Caltrain `/` js | RealWorld `/` rust (warm realms) | RealWorld `/` js | RealWorld article rust (warm) | RealWorld article js |
|---|---|---|---|---|---|---|
| Cold start → first page (ms) | 151 (10 when hot on disk) | 25–32 | 545 | 559 | 522 | 565 |
| Latency p50 / p95 (ms), concurrency 1 | 2 / 2 | 4 / 5 | — | — | — | — |
| Latency p50 / p95 (ms), concurrency 32 or 4 | 46 / 48 | 111 / 118 | 174 / 189 | 176 / 524 | 174 / 186 | 175 / 525 |
| CPU per page (ms) | 2.4–2.5 | 7.7–10.9 | 6.7 (7.9 fresh realms) | 14.2 | 6.3 (7.5 fresh) | 14.2 |
| Throughput (pages/s) | 688 at 32 | 285 at 32 | 22.5 at 4 | 17.2 at 4 | 22.4 at 4 | 17.2 at 4 |
| Resident memory warm → end (MB) | 46 → 71 | 66 → 255 | 81 → 86 | 53 → 72 | 81 → 84 | 57 → 78 |
| Page bytes (raw) | 58,231 | 36,977 | 78,207 | 44,698 | 52,688 | 39,567 |
| Build time, warm (s) | ~1.0 | ~0.5 | — | — | — | — |

What changed in the render host, measured before and after:
- **The accept loop polled** with a 10 ms sleep, which an idle macOS
  process's timer coalescing stretched to 60–70 ms before a request was
  accepted: Caltrain's p50 at concurrency 1 went from 48 ms to 2 ms with a
  blocking accept (a drain wakes it with its own connection). TCP_NODELAY is
  set too; alone it changed nothing.
- **A fresh executor per render** meant a new TLS connection to the API for
  every source: a worker now keeps the executors of renders that settled.
  RealWorld's p50 went from 528–554 ms to 174–181 ms and its CPU per page
  from 16 ms to about 5–8 ms.
- **The first table's CPU was mostly brotli-11** of each re-kept page (a
  `no-cache` request re-renders and re-keeps a `cached` route); `no-store`
  renders without keeping.
- **Warm realms** (`EXACT_RENDER_REALMS=warm`) render the next page with the
  last settled data source instead of a new module realm, about 1.2 ms less
  CPU per page. It relaxes LLP 1048.000 D10's fresh realm per render, so it
  is off by default and is for anonymous pages only.
- (a) as wasm on a JS edge runtime, *estimate*: the render host's core is
  about the size of `app.wasm` (~290 KB brotli) plus the app's data module,
  and its TypeScript sources would need the edge runtime's own engine
  through host calls. Not built.

**(a) against (b).**

| | (a) Rust render host, JS adopts | (b) JS under Bun, JS adopts |
|---|---|---|
| First paint / TTI | FCP 420 ms; runtime up 652 ms (RealWorld, mobile) | FCP 316 ms; runtime up 661 ms |
| Bytes before interactive | page 7.9 KB (inline styles, og metas) + the same `app.js` | page 7.0 KB (classes) + the same `app.js` |
| Adoption code | the same cursor walk, which also strips inline styles and view ids | the same cursor walk |
| Build time (Caltrain, warm) | ~1.0 s (a release render binary: ~52 s cold) | ~0.5 s |
| Adoption correctness | fresh-vs-adopted: every step equal | every step equal |
| A resource that answers later | the checkpoint lists it pending; the runtime asks after adoption | the same, from the runtime's own tickets |
| Documents (1048.003) | the head, canonical, og, robots, status, sitemap, 404 are the render host's | title, description and 404 only; the rest is a gap |
| Single source of truth | two renderers of one plan; the harness guards parity | one implementation renders and adopts |
| Per-request CPU | 2.4 ms (Caltrain), 6–8 ms (RealWorld) | 8–11 ms (Caltrain), 14 ms (RealWorld) |
| Per-request latency | Caltrain 2 ms; RealWorld p95 186 ms | Caltrain 4 ms; RealWorld p95 525 ms |

**Recommendation.** Keep (a) as the default, as ruled. After the fixes
above it is ahead of (b) on every per-request row: about a third of the
CPU, two to three times Caltrain's throughput, a third higher RealWorld
throughput and a flat p95. It is also the renderer native hosts and the
documents code already trust, and the harness holds its pages to the
JavaScript runtime's DOM. Keep (b) for JS edge runtimes and
TypeScript-heavy deployments where a native binary can't run. The remaining
gap is (b)'s documents: head fields, canonical URLs and sitemaps come only
from (a) today.

**Steps 4 and 5: the router and loaded capabilities** (measured after both):
- **Router.** `route/src`'s table, chain and six verbs are ported; history is
  the web host's own `navigation.js`. A synthetic plan (tabs, a deep chain,
  parameters, a query, a modal, refused verbs, history back, reselecting a
  tab) is equal to the Rust runner on all 20 steps; RealWorld's scenario on
  all 21.
- **GPU surfaces** run through the web host's `gpu-glue.js` over the app's
  own `gpu.js`/`gpu_bg.wasm`, fetched after the first painted frame, only
  when a canvas is on the page (as the wasm build loads them: 12.3 + 12.6 +
  102.3 KB brotli for Caltrain). Caltrain's aurora and Weatherlight's sky
  render as the wasm build does; Weatherlight (TypeScript and GPU) is equal
  on all 8 steps.
- **Motion.** `@keyframes` from the plan are in the stylesheet, and CSS
  animations are held to the agent's clock; Motion Gallery's keyframe tiles
  now match. Springs, presence and layout transitions (`motion-glue.js`)
  are not wired.
- **Bytes and time after step 5** (brotli; mobile profile): video player
  6,107 before interactive, 605 ms to answer (wasm 1,976); Caltrain 20,110
  before interactive (page 6,054 + `app.js` 13,655), 640 ms, FCP 380 ms
  (wasm 2,716, FCP 384); RealWorld `app.js` 19,172.

**Conformance, the last full run** (every app with a wasm build, `conform.mjs`):

| | apps |
|---|---|
| every step equal | RealWorld 21/21, Weatherlight 8/8, completion-storm 8/8, video player 4/4, Caltrain 12/12, Typetour 12/12, Carousel 16/16, Sparkline 4/4 and SVG Gallery 12/12 (since 2026-09-29, below); synthetic: router 20/20, regions 8/8, rows 7/7, styles 4/4, composite 3/3, timers 3/3, lists 14/14 |
| state, tree and layout equal; pixels differ | Motion Gallery (animated images) |
| runs, differs | Update Lab (a TypeScript and a Rust source in one app) |
| refused at build, by name | native modules (native-fixture, photo-editor, map-demo, recorder), events `pan` (textflow), `select` (markdown-stress), `cancel` (fieldnotes), `swiperight` (messages-stress), `transformgeometry` (exact-live) |

**What is left, estimated** (*estimates*, runtime bytes brotli):

| Gap | Work | Bytes |
|---|---|---|
| `scrollIntoView` and reorder on a virtualized list (`into_view.rs`, `reorder.rs`) | 2–4 days | ~1–2 KB, in `list.js` |
| The runner's other reserved sources: `exactPage`, `exactDelivery`, `exactSurface`, `exactTime`'s `resolvedLocale` | 1 day | <0.5 KB, loaded |
| Canvas 2D surfaces drawn by a TypeScript source (the bake's recorder in the page) | 1–2 days | loaded |
| Animated images on the agent's clock (`image-glue.js`) | 1 day | loaded |
| Surface records back to sources | 1–2 days | <0.5 KB |
| ~~Dynamic composite rows (clip-path, SVG paint, animation, timeline scope)~~ landed 2026-09-29 (below); left: a dynamic SVG `transform`, marker or `url(#…)` | — | — |
| Text around shapes (`wrap-flow`, LLP 1043.000): the JS target writes it as CSS, which lays out no exclusion (found 2026-09-29) | 1–2 days | loaded (`textflow-glue.js`, `textflow.wasm`) |
| Native modules (NativeProps, custom elements) | 3–5 days | ~1 KB + loaded adapter |
| Events: pan, select, cancel, swiperight, transformgeometry, drags | ~1 week | loaded (`input-glue.js`, `motion-glue.js`) |
| TypeScript and Rust sources in one app | 1–2 days | <0.5 KB |
| Springs, presence, layout transitions (`motion-glue.js`); Bluesky's header hides on a `translate` spring, which the JS target jumps | ~1 week | loaded, 11 KB |
| (b)'s documents: canonical, og, robots, status, sitemap | 2–3 days | build-time only |
| State carried across a dev reload (the loop rebuilds and reloads), delivery (`exactDelivery`, `deliveryActivate`), the rest of the agent (`stages`, plan swap) | 1–2 weeks | agent-only / <1 KB |
| The GPU module built by the JS target's own build (today it is taken from a wasm build) | 1 day | none |

### The tools (2026-09-29)

The tools that drive a web app run what `host/web/build.mjs` makes, so an
app the JS target takes is developed, driven and measured on the runtime it
ships. A JS build's completion marker (`.exact-build.json`) says
`target: 'js'` and lists its files; the local servers serve it as a tree
(`serve.mjs` `buildTreeFile`: no dot path, no symlink), since its chunks are
content-named.

| Tool | Target | How |
|---|---|---|
| Dev loop (`host/web/dev.mjs`) | JS when it takes the app | `host/web-js/dev.mjs`: an edit under the app, `host/web-js` or the base stylesheet rebuilds (`build.mjs --js` into a stage renamed over dist) and every page reloads; a failed build's errors show in the page, which keeps the last good build. ~2.2 s edit → first frame (video player; the compiler, the plan, the Rust data module and the bundles each a process), against the resident wasm loop's ~20 ms and the 100 ms budget row; no slot values carried. `--wasm`, or a refusal, runs the resident loop |
| Agent, web host (`scripts/agent.mjs web`) | what dist holds | a JS dist is served as a tree; the tree reply carries `roots`, the journal a `boot:` line |
| Smoke (`smoke.mjs web`), the app drive and its tests | the default build | the staged-core check is the wasm's only |
| Metrics: bytes, browser startup, dev loop, `--rebuild` | the default build | a JS build reports `app.js` bytes; the dev row times five edits across the reload |
| `serve.mjs` | what dist holds | a JS dist as a tree |

What stays on the wasm target, and why:

- **Delivery** (`deploy.mjs`): its bake is the signed bundle every platform's
  update stream publishes — `exact.json`, the baked plan, the production-trust
  bake receipt — and the web release it publishes is a program the update
  client (`exactDelivery`, `deliveryActivate`) activates. The JS build makes
  none of these (the delivery row above).
- **Native clients on the dev URL** (`build.mjs --url`, `/__dev/open`,
  `exact run`'s live plan): they read the wasm loop's envelope and dev
  generations (LLP 1023), which the JS loop does not serve; it answers them
  with a 404 naming `--wasm`.
- **The smoke's bare-plan fixtures and router sweep**: they swap arbitrary
  plans into a running page (`--plan`, `exact.reload`); the JS target
  compiles one plan ahead of time. `agent.mjs --plan` refuses a JS dist by
  name.
- **The parity smokes**: `motionparity` takes Chrome as the reference for
  animated images on the agent's clock (`image-glue.js`, a gap above);
  `canvasparity`'s apps draw Canvas 2D surfaces from TypeScript, which the JS target refuses.
- **Conformance** (`conform.mjs`): the wasm run is the oracle it compares against.
- **Metrics `--long`'s web bytes**: the wasm's code by capability (LLP 1047 D9).

Found on the way (2026-09-29): the runtime now answers `exactTime`, the
runner's reserved source, itself (`data.reserved`, from `navigation.js`'s
reporters; `resolvedLocale` is `""`, the no-tables answer), which
`auth-fixture`'s `time` resource needed. Its web smoke still stops at the
auth session: the press's held device request (`openAuthSession`) is not in
the JS runtime. A text reading `now()` is not re-rendered by a clock move on
the JS target (the wasm's is). RealWorld's `tap submit` difference was the
harness: its auto-taps compared before a press's fetch (the hosted API)
landed; they now settle first, as scripted steps do.

**Canvas 2D surfaces and declared fonts** (landed 2026-09-29, measured;
brotli):
- **Canvas 2D** (a Rust source's surfaces). The module exports its draws
  beside the ABI (`exact_logic_abi::export_draw!`, `logic/abi/src/draw.rs`:
  op 5, one recorder per canvas generation, the runner's `DataSource::draw`
  unchanged). `rt.js` `c2` keeps each canvas's arguments; `canvas2d.js`, a
  chunk fetched two frames after the first 2D canvas mounts, is the runner's
  half of LLP 1056 D4 (geometry and size generations, causes, frames, fonts)
  over the wasm host's own `canvas2d-glue.js`, unchanged. Cost: 180 B in
  Caltrain's `app.js` (the hook, only where a 2D canvas is), 5,687 B loaded
  (the chunk: 4,014 the glue, the rest the engine and the draw client), 28 B in `rust-data.js`; the module
  grows by the draw code and the recorder it now links, 10.5 KB for
  Caltrain's (35,769 → 46,265, a size build; app.wasm carries the same code).
  Still refused: TypeScript draws; not carried: a Rust draw's `measureText`
  and images (no text engine or image table crosses the seam).
- **Declared fonts** cost the runtime nothing: each face is an `@font-face`
  rule in the build's stylesheet under the web host's family name, with
  `font-display: optional` — the wasm host's own policy (a face not ready in
  about 100 ms stays unused, no swap after paint) — and a preload in the
  shell's head, which a server's early flush sends; a rendered page's head
  drops the render's own copies. Typetour: 171 B of rules and preloads in its shell; the faces themselves are the bytes the wasm host loads too.
- **Caltrain** (mobile profile, 5 cold runs, medians; JS / wasm): FCP
  384–400 / 360–404 ms, runtime up 633–649 / 2,727–2,764 ms, 18,849 /
  337,240 bytes before interactive (page 4,888 + `app.js` 13,961).

**Virtualized lists** (landed 2026-09-29, measured; brotli):
- **What.** `host/web-js/list.js` is the runner's half of LLP 1010 §6,
  1050.000 §6 and 1070, ported from `runner/src/instance/collection`: the
  size index (the sum tree, measurement epochs and tokens), the window led by
  travel, bootstrap rows before the first report, anchoring across data
  changes, first measurements and restored positions, the fill limit and
  retirement, `reachstart`/`reachend` armed by geometry with the end held
  behind the start's requests, `scrollFollowEnd`, authored
  `scrollTop`/`scrollLeft` built before the port moves, and one level of
  nesting with kept positions and pins. Rows are keyed and made fresh, not
  recycled, as the runner's are; the wrappers and spacers are
  `views.rs`'s, so the tree, the boxes and the pixels match. The browser
  half is the wasm host's `collection-glue.js`, unchanged but for handing
  its report's facts to the JS runner as values beside the wire bytes.
- **Cost.** `list.js` is 6.4 KB, in the module only when the plan has a
  virtualized list; `collection-glue.js` 4.9 KB, fetched after the first
  paint, as the wasm build fetches it. An app without a list pays 199 B in
  `rt.js` (the video player, 4,751 → 4,950 B `app.js`): the commit hook,
  the no-op count an edge reads, and authored scroll offsets, which every
  app needed (a dynamic `scrollTop` was written as an attribute).
- **Conformance.** Carousel 16/16, its scripted scroll of 25,000 cards and
  its feed of nested strips and inboxes; synthetic lists 14/14, a long list
  scrolled to both edges (each grows it; the start's rows come above,
  anchored), an authored jump and a transcript following its end. A press
  on a page with a list is compared once both have settled, since a list
  builds in the frames after it. Found: a wheel-scrolled list whose rows
  differ from their estimate is not repeatable across two runs, on either
  target: which rows a moving port builds ahead of itself, and so which it
  has measured, is the frame clock's. The fixture's long list uses rows at
  its estimate.
- **Not carried**, refused by name: `scrollIntoView`, reorder
  (`reorderdrop`), a dynamic `virtualized`.

**Bluesky** (outside the repo, `EXACT_APP_DIR`; 2026-09-29): it builds on the
JS target and its timeline, a thread, a profile, sign-in (the `demo`
account) and notifications work in a browser.
- **What it took** (each a refusal or a failure it met): apps outside
  `apps/` (`host/web/build.mjs` no longer refuses them; the Rust module is
  built from the app's own data crate); the reserved source
  `exactViewport` (`facts.js`, the web host's own readings, re-answered on
  each resize; only where declared), and both it and `exactTime` answered
  by the page even where the bake compiled a value; symbol images (`symbols.js`, the web host's
  masks; a bound source's roles come from the plan's strings); the `scroll`
  event, and `refresh`, which the web does not deliver (no pull to refresh,
  as in the wasm host); dynamic `translate`, `line-height` and
  `aspect-ratio`, each one declaration as the author wrote it; a
  paragraph's inline runs as text nodes in the agent's tree. On Bluesky's
  side: `contains` → `includes`, the plan's `Items`, the lock.
- **Conformance** (a local scripted run, not the lane's: its first screen is
  the live network, and two loads a moment apart can get two Discover
  feeds): with the same feed, 9 of 11 steps equal. The two that differ are the
  spring gap: scrolling hides the header on a `translate` spring, and the
  wasm target's `clock settle` runs its clock across the spring, where a
  250 ms timer fires.
- **Measured** (realworld-bench's method and launcher, its `load`
  scenario: mobile profile, a cold browser each run, 5 runs, medians; both
  client-rendered, served brotli-11; the first post is Discover's, from the
  live network):

  | | JS | wasm |
  |---|---|---|
  | First contentful paint | 760 ms | 2,988 ms |
  | Runtime up | 750 ms | 2,976 ms |
  | Data module ready | 2,201 ms | 3,423 ms |
  | First post on screen | 2,726 ms | 4,298 ms |
  | Code before runtime up | 42.6 KB (page 5.4 with its stylesheet, `app.js` 37.7) | 469.3 KB (`app.wasm` 453.4, `glue.js` 17.0) |
  | Code before the data module is ready | 196.2 KB (+ the Rust module 122.1, the plan it binds 27.3) | 495.3 KB |

  The JS target's remaining weight is the data seam's: Bluesky's AT
  Protocol client as its own wasm, bound with the whole plan. Binding it with
  only what it reads (its sources and shapes) is a lever not yet taken; the
  module itself is the app's code.

**Dynamic composite rows and SVG** (landed 2026-09-29, measured; brotli):
- **What.** A dynamic row whose grammar is CSS's own is one declaration as
  the author wrote it (style.rs `style_writes`): `clip-path` (only `none`,
  `url()` and `path()`, the kernel's grammar; any other shape is unset),
  `shape-outside`, SVG `fill`/`stroke` and dashes, `filter`,
  `transform-origin`, `paint-order`, gradients; `animation` over the plan's
  `@keyframes` (paused again under an `animation-timeline`, whose shorthand
  would reset the play state); the timeline rows as css.rs writes them
  (`timeline-scope` with its `--exact-timeline-scope`, the drag and
  animation timelines' custom properties); an eased `transition`. Refused by
  name: a dynamic SVG `transform` (SVG's syntax, which the kernel restates as
  CSS), a marker, and any value that can be `url(#…)` (the kernel scopes ids
  per instance), `animation` or `transition` on a node whose press feedback
  scales (`--exact-scale`), and a `transition` that can be a spring.
- **Found and fixed.** The compiler never linked the host's grammars
  (`exact_web::link`), so a *static* `animation`, `clip-path`, `filter`,
  gradient or drag timeline row was left out of the stylesheet: Motion
  Gallery's keyframe tiles ran on no rule (its stylesheet +148 B). SVG
  elements were made in SVG's namespace by a tag list that lacked `defs`,
  gradients, `stop`, `clipPath`, `mask`, `marker`, `pattern`, `use`,
  `symbol`, `text` and the filters: the compiler now says (`hs`, from the
  node type), which took 51 B off every app's module (the video player
  4,954 → 4,903 B `app.js`). A `symbol`'s content carries its rows inline
  too, since Chrome styles `use` clones without the page's class rules. The
  agent held every CSS animation at the clock's time from zero; it now
  keeps each from the time it began, author-paused ones at their own, and
  `clock settle` runs the clock to where the last one ends, as the wasm
  host's `animationClock` does (agent-only bytes).
- **Cost.** Only a plan with such a row pays: a `S(e, prop, "", f)` per row,
  plus a small mapping function for `clip-path`, `animation`, the timeline
  rows. Sparkline 15,262 B `app.js`, SVG Gallery 12,331 B (page 4,186).
- **Conformance.** Sparkline 4/4, SVG Gallery 12/12, and a synthetic
  `composite.contract` (over SVG Gallery's dist, which links the grammars)
  3/3; both apps join the async lane's run.

## 8. Rulings and open questions for Charlie

**Rulings.**
- *Pre-rendering* (Charlie, 2026-09-28): "keep both options available to
  allow for different deployment scenarios, but Rust should be the
  primary/default." The Rust render host writes pages by default; the JS
  render (`render.mjs`) is the option.
- *The name and the default* (Charlie, 2026-09-28, relayed): this is exact2's
  web support, not "Exact 3" — a new compile target for the same Contract.
  The web build uses it by default when an app qualifies and otherwise builds
  the wasm target, printing the refusal; a flag forces either. The wasm target
  on the web retires once the gaps in §7 close (they are listed in
  `rules/DEFERRED.md` too). The conformance harness is a required check: it
  runs in the async lane (`conform.mjs --strict`), since RULES keeps the five
  blocking checks under a minute and this run takes minutes and a network.
  Questions 1, 2 and 6 below are answered by this ruling.

**Open.**

1. **The name.** Is this Exact 3, or an exact2 web target? Nothing native
   changes, which argues for a target; a second runner is a big enough change
   to argue for a name.
2. **The Rust wasm path.** Does it stay as a fallback, for apps using a
   capability the JavaScript runtime lacks (D5's refusal) and as an oracle in
   the browser? Or is it deleted once the JavaScript runtime covers the apps
   ("delete; don't deprecate")?
3. **DEFERRED.** Admitting a JavaScript runner above the seam is a trade. What
   comes off? A candidate: LLP 1047's remaining core diet and staging lanes
   (D9's report, the data split), which this makes moot on the web.
4. **A byte budget in RULES.** Should the time-budget table gain "First
   interaction, web, Caltrain: ≤ 25 KB brotli", tracked per commit like the
   other rows and never blocking? The spike's number would set it.
5. **The working set** is at 15 of 15, so this document isn't linked into
   `llp/current/`. Should it be, and what leaves?
6. **New apparatus.** The spike adds a backend, a runtime and a runner mode for
   `agent.mjs`, on its branch. Landing any of them on main needs your yes
   under RULES §Agents.
