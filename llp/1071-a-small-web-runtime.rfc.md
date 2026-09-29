# LLP 1071: A small web runtime — Contract compiled to JavaScript, the DOM as the tree

**Type:** RFC
**Status:** Draft. Charlie approved writing it and running the spike on 2026-09-28. Nothing here is decided.
**Systems:**
- Contract: a second backend, from the validated plan to JavaScript (`contract/`).
- Web host: a JavaScript runtime beside `exact-web` (`host/web`); the glue, capture and adoption.
- Build: `host/web/build.mjs`, a build-time stylesheet from `css.rs`.
- Render server (LLP 1048): unchanged as the producer of documents; the checkpoint gains instance state for D6.
- Agent (LLP 1012): the nine operations over the JavaScript runtime, in a development module.
- Native hosts: none.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** the spike (§7): Claude (Opus 5.5), branch `exact3-web-spike`, from 2026-09-28. Anything past the spike waits for Charlie's rulings (§8).
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
stop emulating the browser and be it.

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

Branch `exact3-web-spike`. It measures the claim; it lands nothing on main.

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
JavaScript renders the same page under Bun (`host/web3/render.mjs`) for JS
edge runtimes and TypeScript-heavy deployments.

**What was built.**
- `exact-web3 js <app.contract | baked app.plan>`: the plan's bytecode to
  JavaScript (structured forward jumps as labeled blocks), the view as DOM
  construction, static rows to a class stylesheet computed by the web
  host's own `tag_for`/`props_for`/`css_text`/`host_css` (a new
  `exact_web::host::template::parts`), dynamic style units read from
  `css_text` itself.
- `host/web3/rt.js`: signals, commits with rollback on refusal, a
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
- `host/web3/conform.mjs`: the conformance harness (§4 below), and
  `host/web3/bench.mjs` / `render-bench.mjs` for the numbers here.

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
| every step equal | RealWorld 21/21, Weatherlight 8/8, completion-storm 8/8, video player 4/4; synthetic: router 20/20, regions 8/8, rows 7/7, styles 4/4, timers 3/3 |
| state, tree and layout equal; pixels differ | Caltrain (its Canvas 2D line map), Motion Gallery (animated images) |
| runs, differs | Typetour (1 px text: declared fonts not loaded), Update Lab (a TypeScript and a Rust source in one app), Carousel (a virtualized list rendered whole) |
| refused at build, by name | native modules (native-fixture, photo-editor, map-demo), dynamic `line-height` (exact-live, llp, markdown), dynamic SVG `fill` (svg-gallery), dynamic `clip-path` (reflow), dynamic `animation` (sparkline), `timeline-scope` (interaction-gallery), events `pan` (textflow), `select` (markdown-stress), `cancel` (fieldnotes), `reachstart` (messages-stress) |

**What is left, estimated** (*estimates*, runtime bytes brotli):

| Gap | Work | Bytes |
|---|---|---|
| Canvas 2D surfaces: a `draw` op on the logic ABI, `canvas2d-glue.js` reused | 3–5 days | ~0.5 KB core; 3.8 KB loaded |
| Virtualized collections: the window, measurement and anchoring engine | 1–2 weeks | 4–6 KB, loaded |
| Declared fonts (the web host's font loading reused) | 1–2 days | <1 KB |
| Animated images on the agent's clock (`image-glue.js`) | 1 day | loaded |
| Surface records back to sources | 1–2 days | <0.5 KB |
| Dynamic composite rows (line-height, clip-path, SVG paint, animation, timeline scope) | 2–3 days | ~1 KB |
| Native modules (NativeProps, custom elements) | 3–5 days | ~1 KB + loaded adapter |
| Events: pan, select, cancel, reachstart/end, drags | ~1 week | loaded (`input-glue.js`, `motion-glue.js`) |
| TypeScript and Rust sources in one app | 1–2 days | <0.5 KB |
| Springs, presence, layout transitions (`motion-glue.js`) | ~1 week | loaded, 11 KB |
| (b)'s documents: canonical, og, robots, status, sitemap | 2–3 days | build-time only |
| Dev reload and state carry, delivery (`exactDelivery`, `deliveryActivate`), the rest of the agent | 1–2 weeks | agent-only / <1 KB |

## 8. Open questions for Charlie

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
