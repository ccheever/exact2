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

**Activation, first form (the spike's).** The policy stays `idle` or first
input, with replay (`1048.000:247-250`). The runtime runs the route's
components against the checkpoint, claiming existing nodes by `data-s`
instead of creating them. A mismatch renders fresh once, as today. The
difference is ~20 KB to fetch instead of ~330.

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

pending: branch exact3-web-spike

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
