# LLP 1112: A URL under a state, as an image

**Type:** RFC
**Status:** Draft r3 (2026-10-07): round 2 by Astra (max) and Grok 4.7 (xhigh), both NOT READY; r3 makes D1a a procedure and narrows to two first slices. r1 and r2 are reviewed in `llp/reviews/1112-a-url-under-a-state-as-an-image.{astra,grok}.md`; §10 lists what r3 took.
**Renumbered (2026-10-08):** drafted and reviewed as LLP 1106, a number origin had meanwhile given another document; its reviews keep the old numbers (1105→1111, 1106→1112, 1107→1113). Code citations were checked at `2ce193976`, 542 commits behind the commit that adds this file.
**Parked (2026-10-07), after round 3** (Astra and Grok 4.7 both NOT READY; their round-3 sections are in the review files). The blockers in D1a (the two clocks, timers fired by a later input, image work started by the final paint, the round cap's result) moved to LLP 1113 (settled and the two clocks). r4 replaces D1a with a citation of LLP 1113 D3 under D5's picture and test columns, then takes the remaining round-3 items: `answer fetch`'s native insertion point after origin admission, `host/web/faults.js` and `http-body.js:184` on the web, and installing the table outside agent mode.
**Systems:** Slice 1: the authored-test grammar (`contract/syntax/src/parser/steps.rs`), the runner's fault table (`runner/src/runner/faults.rs`, LLP 1103 D1), the native executor core (`host/apple/src/executor_core.rs`), the JS target's admission (`host/web-js/admission.js`), the wasm glue (`host/web/module-glue.js`), the drivers (`scripts/agent-launch.mjs`, `scripts/agent-test.mjs`). Slice 2: the Linux host (`host/linux`: picture mode, the launch record, the settle procedure, pinned fonts, the refusal report), `scripts/exact.mjs render`, `scripts/fixtures/fonts/`. Later slices are direction only (§5).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06 (r1, r2); 2026-10-07 (r3)
**Implementer:** none yet. This document proposes; it specifies nothing until it has one.
**Related:** LLP 1015 (the Linux host and its CPU painter), LLP 1048 / 1048.000 (pages at build and per request), LLP 1038 (a location is a URL), LLP 1069.000 (device facts by their web names), LLP 1069.007 D2 (a production bake never enters agent mode), LLP 1069.009 (an input is a runner call), LLP 1018 (the store), LLP 1097 (background storage work), LLP 1103 (`fail fetch`; D3 makes its facts development-only), LLP 1012 / 1079 (the ten operations), LLP 1019 (declared fonts), LLP 1055 D10 (an infinite animation never settles), LLP 1093 (paged media still out), LLP 1071 (the 2026-09-28 render ruling). Siblings: LLP 1101.003 (the command line) and LLP 1111 (apps as tools for agents) start an app at "a URL under a state" (D1) and use D1a with the differences D1a names.

## Summary

Given an app, a URL and a state, produce the picture the app shows. The uses
are social cards (`og:image`), documentation screenshots, a mailed report.

The Linux host's one-frame mode already boots a location, waits up to 500 ms
for images, paints with the CPU painter and writes a PNG
(`host/linux/src/app.rs:547-578`). It does not pump data, takes device facts
only in agent mode, and runs with the app's own grants and storage. (A debug
`caltrain-linux` built 2026-09-21 wrote a 2400×1260 PNG in 0.27–0.36 s warm on
2026-10-06. That is a historical observation, not this commit's cost.)

r3 specifies two slices and leaves the rest as direction:

- **Slice 1, `answer fetch`** (§3): a test launch line that answers a fetch
  from a file, after the grant admits it. Useful to tests on its own.
- **Slice 2, one local PNG** (§4): `exact render caltrain / --size 420x860`
  in development, settled by D1a, refused or reported by what it cannot draw.

The shared terms are D1 (the shape of a state), D1a (the settle procedure and
"publishable") and D9 (the launch record).

## 1. Motivation

The framework emits `og:image` when a page's head names an image
(`host/web/src/page.rs:87-103`), but the author draws that image elsewhere.
Caltrain names none (`apps/caltrain/app.contract:171`). A picture is not one
paint away from a rendered page: the render host does not lay out
(`host/render/src/lib.rs:42-43`), so a picture is a fresh boot, layout and
paint. The agent's `screenshot` makes one, but only in agent mode, which a
production build refuses (`host/linux/src/app.rs:146-151`).

## 2. Decisions

### D1 — "A URL under a state" is a launch: a location, the launch facts, and optionally steps

A state is the inputs that lead to a picture, given as runner calls (LLP
1069.009 D1), not a snapshot of the runner.

| Part | Test launch line | `exact render` flag | Launch record (D9) |
|---|---|---|---|
| location | `location "/talks/42"` (new) | positional | `location` |
| viewport | `size 1200x630` (exists) | `--size 1200x630` (exists) | `size` |
| scale | `scale 2` (new) | `--scale 2` (new) | `scale` |
| device facts | `prefer <name> <value>` (new) | `--prefer <name>=<value>` | `prefer {name: value}` |
| locale, time zone, epoch, seed | `locale`, `time-zone`, `epoch`, `seed` (exist) | `--locale`, `--time-zone`, `--epoch`, `--seed` (exist) | `locale`, `timeZone`, `epoch`, `seed` |
| answers | `fail fetch` (exists), `answer fetch` (new, §3) | `--fail-fetch` (exists), `--answer-fetch` (new) | `answers` (development only) |
| steps | the test's steps | `--test <file>` | none |

"Exists" means `steps.rs:716-724` parses it as a launch line or
`scripts/agent-launch.mjs:126-152` as a flag. Omitted fields follow D9's table.

- **Duplicates.** A test may not repeat a launch line (`same_launch`,
  `steps.rs:109-113`, `:745-751`). r3 keeps that rule and extends it: two
  `prefer` lines for the same fact are duplicates, and for different facts
  they are not. A `fail fetch` and an `answer fetch` with the same prefix are
  duplicates. Under `exact render --test`, a flag overrides the test's launch
  line of the same name.
- **No store snapshot.** LLP 1018's plain tier is unbuilt
  (`llp/1018-durable-client-state.rfc.md:137`), `--storage <name>` is a scratch
  store kept between drives, and apps keep data elsewhere (Fieldnotes in
  SQLite, `apps/fieldnotes/app.ts:29`). Seeded data comes from steps that run
  the app's own actions (LLP 1111 seeds its notes so) or from `answer fetch`.
- *Rejected: reserved query parameters on the app's URL.* The router owns the
  query, and a URL cannot carry a drive.

### D1a — "Settled" is a procedure, in milliseconds

D1a is one procedure, modelled on the agent's `clock data` and `clock settle`
on the Linux host. Each step names the code it reuses. No step calls the
runner's `advance_timed` (`host/linux/src/host.rs:972-975`), so no app timer
fires. The caller's deadline (D11) bounds the whole procedure.

**(a) Activation.** Paint one frame, then call `first_pixel`, which needs a
clean, successful frame (`host/linux/src/presenter/delivery.rs:104-113`), as
`EXACT_SHOT` does (`app.rs:553-554`). Wait while `data_activating()`
(`delivery.rs:117`), sleeping 20 ms, painting when dirty, and calling
`first_pixel` again, as `land_data` does (`host/linux/src/agent.rs:740-750`).
This is the render host's `preload`/`activate`/`data_ready`
(`host/render/src/lib.rs:720-740`), done the Linux host's way. A module that
fails to activate makes the picture not publishable.

**(b) Data, to a fixed point.** Repeat, at most 16 rounds, until a round
changes nothing:

1. `pump` every round, not only while a request is pending. It applies
   announced topics even when no reply is queued
   (`host/linux/src/presenter.rs:1316-1324`, then `Host::fulfill_all` applies
   them first, `host/linux/src/host.rs:801`). A data module's `changed()`
   reaches the tree before the check.
2. Wait for replies while `has_pending` (`wait_for_replies`, `agent.rs:934-945`).
3. `land_then`, which also runs a due queued `next` with timers off
   (`host.rs:964-967`; `runner/src/runner/commit.rs:339-345`).
4. `sync_surfaces` (`host/linux/src/surfaces.rs:1050`), so a Canvas 2D bitmap
   exists before the paint (`host/linux/src/paint.rs:995-1003`).
5. The check: nothing pending, `background_operations() == 0`
   (`runner/src/runner/background.rs:126-130`), and no node whose
   `accessibilityBusy` prop is true. That prop is `aria-busy` lowered
   (`contract/lower/src/tags.rs:532`; a `bool` in `kernel/tables/schema.json:320`).
   No host reads it today, so the walk over the kernel tree is new work.

**(c) Motion, to a fixed point, in ms.** Let `t` be `settle()`
(`agent.rs:691-698`): the maximum of the engine's `settle_time() × 1000` with
the press settle (`host.rs:651-662`) and `group_settles_at()`, which is
already in ms (`host/linux/src/presenter/group.rs:424-436`). While `t` exceeds
`host.now()`, call `Presenter::tick(t)` (`presenter.rs:1399-1409`, then
`Host::tick`, `host.rs:1175-1178`), then `sync_surfaces`, and recompute. A
seek can start motion, so this loops, at most 16 rounds.

- **Infinite animations move too.** `Host::tick` advances one engine time, and
  every animation samples it (`motion/src/engine.rs:524`;
  `motion/src/engine/animate.rs:245`). An infinite animation is therefore
  sampled at the final sought time, not at the data-settled clock, and the
  report says `motion: "infinite"`. `settle_time` omits infinite animations
  (`engine.rs:672-678`), so knowing that one runs is a new engine query.
- **The capture moves the session's clock.** `Host::tick` sets `now_ms`
  (`host.rs:1176`), which later commits read; the engine cannot go back
  (`engine.rs:553`). After a screenshot in `exact render --test`, the next step
  sees the clock at the sought time. Timers that came due by then fire at the
  next step that moves the clock (`clock`). A test that needs the unmoved clock
  takes its screenshot last.

**(d) Images.** Wait until the loader's `pending()` is false
(`host/linux/src/image.rs:412-443`, through `wait`, `:481-497`), applying
reports as `wait_images` does (`presenter/images.rs:13-34`). A report that
changed layout (`set_intrinsics`, which re-runs layout and queues collection
work) returns to (b). Being done means decoded or failed, not "a report
arrived".

**(e) The deadline.** At the caller's deadline the procedure stops where it
is and reports `settled: "deadline"` with the step it stopped in.

**Publishable** is a separate predicate, checked after D1a:

- no resource the runner marks failed: the table `failed(x)` reads
  (`failed_args`, `runner/src/runner.rs:376`), which needs a public read;
- no image that failed or was refused (`Refusal::DecodeFailed`, `QueueFull`,
  `Budget`, `Prepared::Failed`; `image.rs:222`, `:311`, `:439`);
- data-settled, not stopped at the deadline;
- the head's status is absent or 200, carried into the report as `status`, as
  the render host carries it (`host/render/src/lib.rs:80-92`);
- no visible unsupported node (D2) and no missing glyph (D3).

Settled says the work stopped. Publishable says the picture is the one the
author meant.

**The siblings' differences.** LLP 1101.003 runs (b) on the wall clock and
fires timers as they come due, and it has no motion engine, so (c) is empty.
An LLP 1111 call does not settle the whole tree: it tracks only the requests
and continuations its own invocation started.

### D2 — The painter is the Linux host's CPU painter; Chrome stays the oracle

The CPU painter makes the picture, rather than the GPU painter (a shader
compile, determinism only within a band) or a browser on the server. For one
renderer build, font set, asset set and launch, it gives the same bytes on
every machine (`host/linux/src/raster.rs:1-3`).

| Content | On the CPU painter |
|---|---|
| portable `symbol:` images | drawn (`host/linux/src/paint/svg.rs:71-91`) |
| `sf/` symbol names | empty box (`svg.rs:76`) |
| Canvas 2D | drawn when its bitmap is present (`paint.rs:995-1003`; D1a (b)4) |
| GPU-module canvases | not drawn: `native()`, a no-op (`paint/backend.rs:67-74`) |
| `http(s):` images | not loaded (`host/linux/src/image/assets.rs:55`, `:91`) |
| `data:` images | loaded, to their bound (`assets.rs:97`) |
| JPEG | refused off Android (`host/linux/src/image/jpeg.rs:206`) |
| video, iframe, native views | omitted (`paint/native.rs:1`) |
| a 3D transform | refused (`spatial`, `paint.rs:867`) |

**Refusal uses the painter's own visibility.** The walk already resolves
`display`, inherited `visibility`, the transform, the clip and accumulated
opacity (`paint.rs:855-929`; opacity 0 swaps in an `Unpainted` backend,
`:929-931`). Where it reaches a node it cannot draw (`native()`, `spatial`),
the walk records the node if its backend draws, `paints()` holds, and its
transformed box clipped by `clip_rect` meets the viewport. That record is the
report's `flat: […]`. There is no second visibility model.

A card that needs a network image has its data module produce a `data:` URL.
Picture mode does not load `http(s):` images.

**Chrome stays the oracle.** A pixel mode for `host/web-js/conform.mjs`,
using D3's fonts in Chrome and LLP 1015 §8's metric (`:684`), is new
async-lane apparatus. It needs Charlie's approval, and it comes before anyone
relies on a public picture (§5).

### D3 — The picture's fonts are pinned

`scripts/fixtures/fonts/` holds Latin subsets of DejaVu Sans Book and Bold.
Two more subsets from the same release are added: DejaVu Serif Book and
DejaVu Sans Mono Book.

| Generic family | Pinned face |
|---|---|
| `sans-serif`, `system-ui`, `ui-sans-serif`, `cursive`, `fantasy`, `ui-rounded` | DejaVu Sans |
| `serif`, `ui-serif` | DejaVu Serif |
| `monospace`, `ui-monospace` | DejaVu Sans Mono |

A picture's fonts are the plan's declared faces (`host/linux/src/text/catalog.rs:191-196`)
and the pinned faces, never the machine's: picture mode skips the system scan.

A glyph is looked up in the run's `font-family` list, then in the pinned face
for its generic family, then in DejaVu Sans. Today the catalog falls back to
the platform instead (`catalog.rs:111`), so this order is new work. A glyph no
face has draws as the missing-glyph box and is counted (`missingGlyphs`). Emoji
and CJK are known gaps (Q3).

### D4 — PNG of the viewport

`size` is the viewport in CSS pixels and `scale` is the device pixel ratio.
Picture mode refuses a scale outside (0, 4] or a side over 4,096 device
pixels by name; today a bad `EXACT_SCALE` warns and draws at 1 (`app.rs:56`).
The picture is the viewport, scrolled to the top, after D1a. Whole-page
capture is direction (§5).

### D5 — PDF (direction, §5)

### D6 — The command line: `exact render` (slice 2, §4)

### D7 — Pictures at build (direction, §5)

### D8 — Pictures per request (direction, §5)

### D9 — The launch record: facts a production build accepts

The facts D1 needs are read only in agent mode today: locale, zone, epoch and
seed under `EXACT_AGENT=1` (`host/linux/src/zone.rs:28-40`, `:82-95`), and
`prefer` as an agent operation. A production bake drops agent variables
(`app.rs:146-151`).

**`EXACT_LAUNCH=<path>`** names a JSON file holding D1's launch-record column.
It is the name for slice 2; the server's use of it is direction. A
development example:

```json
{ "location": "/", "size": "420x860", "scale": 2,
  "prefer": { "prefers-color-scheme": "dark" },
  "answers": [{ "prefix": "https://api.example/talks/42", "file": "talk-42.json" }],
  "picture": { "out": "/tmp/x.png" } }
```

- **The `picture` object is what enters picture mode.** Without it, a launch
  record only sets facts. `EXACT_SHOT` stays the old one-frame path.
  (`EXACT_LAUNCH_URL`, which already exists, is a different variable.)
- **It never turns on agent mode.** It opens no agent channel, and
  `EXACT_AGENT` stays off. LLP 1069.007 D2 still ignores `EXACT_AGENT` in
  production; the record carries facts, not operations.
- **Unknown fields fail.** An unknown field, or a value out of range, is a boot
  error by name.
- **`answers` is development only** (LLP 1103 D3). A production build refuses
  a record that has `answers`.

**Omitted fields, by launch mode:**

| Field | Picture (`exact render`, build) | Test (`--test`, agent) | Command line (LLP 1101.003) | Named state (direction) |
|---|---|---|---|---|
| `locale` | `en-US` | `en-US` | `LC_ALL`, then `LC_MESSAGES`, then `LANG`; `C` and `POSIX` are `en-US` (`zone.rs:111-132`) | `en-US` |
| `timeZone` | `UTC` | `UTC` | `TZ`, else the system zone (`zone.rs:96-100`) | `UTC` |
| `epoch` | `1767225600000` (`zone.rs:35`) | the same | the wall clock | fixed by the state; never the server's clock unless the state says so, and then not in the cache key |
| `seed` | `1` | `1` | system entropy (`zone.rs:93-95`) | `1` |
| `prefer` | the agent's `LAUNCH_MEDIA` (`scripts/agent-prefer.mjs:8`) | the same | the terminal's (LLP 1101.003 D6) | the state's, else `LAUNCH_MEDIA` |
| `answers` | live (development may set) | live | refused | refused |

A picture's defaults are fixed, so an omitted field never follows the
machine, and the same launch gives the same key. The command line column is
the person's launch, which `launch_place` already builds in its non-agent
branch (`zone.rs:82-110`).

### D10 — The anonymous mode (direction, §5)

### D11 — Deadlines

For slice 2, `exact render` sets a cooperative deadline in the child
(`--deadline`, default 10,000 ms, covering D1a, the paint and the encode). The
child's watchdog interrupts a running source call there: Hermes stops through
`asyncTriggerTimeout` (`js/src/shim.cc:348-349`), and a Rust source runs to
the end of its call. The hard deadline is the cooperative one plus 500 ms,
enforced by `exact render` itself: it spawns the child, kills it at the hard
deadline, and exits 124. The server's deadlines are direction (§5).

## 3. Slice 1 — `answer fetch`

```text
test "talk 42, from a fixture"
  answer fetch "https://api.example/talks/42" with "fixtures/talk-42.json"
  …
```

It is a launch line and a driver flag (`--answer-fetch <prefix>=<file>`), and
development only, like `fail fetch` (LLP 1103 D3). It is not a drive
operation.

- **One table.** It is a second kind of entry in the runner's fault table
  (`runner/src/runner/faults.rs`). The prefix is matched against the whole URL,
  query included, as `fail fetch` matches it. The longest prefix wins across
  both kinds. An answer applies to every matching fetch; there is no `times`.
- **The grant comes first, at each site:**
  - **Native (Linux and Apple).** The presenter asks the runner for a fault at
    hand-off (`fault_dispatch`, `runner/src/runner/faults.rs:205-221`, called
    from `host/linux/src/presenter.rs:701`). Today its work runs before the
    bindings check (`host/apple/src/executor_core.rs:850-853`). An answer
    returns a new dispatch, which the core substitutes only after `bindings`
    resolves and `timeout_refusal` passes (`:854-860`), where a live request
    would go to the transport. Native admission is by origin (LLP 1103 D1), so
    an answer is admitted exactly as a live request to that origin would be.
  - **JS target.** `fetchWith` (`host/web-js/admission.js:28-44`) consults the
    table after `admitsNetwork` and before `browserFetch`.
  - **Wasm web host.** `fetchEarly` (`host/web/module-glue.js:51-56`) returns
    `null` for a URL the answer table matches, as it does for a fault, so a
    matched GET is never started live. The later host request is answered at
    the same place `fail fetch` is.
- **The response.** Status 200; `Content-Type` from the file's extension
  (`content_type`, `host/render/src/files.rs`); the body is the file's bytes;
  the response URL is the request's; no other headers. The source's own
  parsing runs.
- **The file.** The path is relative to the test file, or to the working
  directory for a flag, and must lie inside the app's directory. The file is
  read once at launch. It fails the launch by name if it is missing, escapes
  the app's directory, or is larger than 64 MiB (`MAX_BODY`,
  `executor_core.rs:26`). The cap is checked by the driver for every executor.
- **Live fetches are counted.** An unmatched fetch goes live and the report
  counts it (`live: <n>`). A picture with live fetches does not claim to be
  reproducible.
- **Tests:** the same test on the web (JS target), wasm and Linux; an answer
  for an ungranted origin is refused on all three; `fetchEarly` sends nothing
  for a matched URL; a duplicate prefix is a parse error.

## 4. Slice 2 — one local viewport PNG

```sh
bun scripts/exact.mjs render caltrain / --size 420x860 --scale 2 --out home.png
bun scripts/exact.mjs render caltrain --test apps/caltrain/pictures.test.contract --strict
```

- **What runs.** `render` is a new verb (`scripts/exact.mjs` has none). It
  builds or reuses the app's Linux executable in `host-dev` and runs it with a
  launch record that holds a `picture` object (D9). With `--test`, it runs the
  test the way tests run today, on the agent's headless Linux host, and
  settles by D1a before each `screenshot` step.
- **The fixture.** Caltrain has one route, `/`, and a `notfound`
  (`apps/caltrain/app.contract:11-13`). At boot `sky` is `some("#05081a")`
  (`:63`), so `Sky` draws a GPU canvas filling the screen (`:187-188`). That
  canvas is a visible unsupported node. No `prefer` fact turns it off: the
  person's "Toggle the sky" button does (`toggleSky`, `:134-135`; the button's
  `testId="sky-toggle"`, `:276`). So the acceptance case is two pictures:
  1. `render caltrain /` without `--strict` writes the picture and reports
     `flat: ["sky"]` and `publishable: false`.
  2. `pictures.test.contract` runs `tap sky-toggle`, then `screenshot`, under
     `--strict`. With the sky off, the page is a plain scroll (`:190-192`). The
     line map is a Canvas 2D surface drawn by the data crate
     (`apps/caltrain/data/src/lib.rs:253`, `:264`), drawn after (b)4. The deck
     canvas sits under `when deck`, which is false (`:59`, `:317-318`). The
     ticker is `every(1000)` (`:137-138`) and does not fire, so `nowMs` stays
     `1787915400000` (`:54`) and the boards are reproducible. This picture
     must be publishable.
- **The report.** One JSON line: `path`, `w`, `h`, `settled` (`complete` or
  `deadline`, and the step), `publishable` with its reasons, `status`,
  `motion`, `flat`, `missingGlyphs`, `live`, and the font set's digest.
- **Exit codes.**
  - 0: written; publishable, or `--strict` not given.
  - 1: not publishable under `--strict`; nothing is written.
  - 2: a usage or launch error, such as a bad record, a missing fixture, a
    scale out of range, or an unknown field.
  - 124: killed at the hard deadline (D11); nothing is written.
- **Outside the repo.** An app that `exact new` made has no `linux/` crate
  (`game/new.mjs:188-255`). Slice 2 runs on Caltrain only; the template is Q4.

## 5. Later slices (direction, not specified)

These are where the RFC is going. None is specified, and each has the open
problem a review named:

- **D5, PDF:** a separate artifact over the painter's walk, one tall page.
  Open: the backend trait's output type, text cluster ranges, CSS-px-to-pt,
  raster fallback over the backdrop. Paged media stays out (LLP 1093).
- **D7, build pictures** (`host/web-js/build.mjs:553`): open are a
  collision-free URL-to-file codec (`/a` and `/a.png/b` collide), percent
  aliases and encoded separators, the root form's containment, publishing a
  head image only after its picture succeeds, `--render js` (LLP 1071 keeps
  it), and building anonymous mode first.
- **D8, per-request pictures:** open are serving a built file only for its
  exact location (query included), the cache key's asset and deployment
  digest, server time against cache lifetime, a status table that keeps
  `head.status` (410, 503), 500 vs 503 for a killed child, the artifact
  handoff (which plan, module, assets and fonts the child loads, and how they
  are matched), and hosting with a front door (LLP 1048.000 binds loopback).
- **D10, anonymous mode:** `Anonymous` moves to `exact-data`, with no storage,
  no app files and no development behavior. Open: a private-address refusal at
  the connection layer, covering every redirect hop and resolved address
  (loopback, link-local, RFC 1918, CGNAT, IPv6 ULA, IPv4-mapped, cloud metadata),
  and whether a grant can opt in. No such check exists today.
- **Memory:** an aggregate accounting of frames, layers, decoded images
  (512 MiB at 4096² under today's image budget, `image.rs:100`), canvases,
  encode buffers and cached output, before any server claim.
- **Whole-page capture:** the document extent uses root frames
  (`host/linux/src/presenter.rs:803`), LLP 1010 §3's deviation.

## 6. What changes, where (slices 1 and 2)

| Where | Change |
|---|---|
| `contract/syntax/src/parser/steps.rs` | `answer fetch … with …`, `location`, `scale`, `prefer`; `same_launch` by prefix and by fact |
| `scripts/agent-launch.mjs`, `scripts/agent-test.mjs` | the flags; the 64 MiB file cap; `live` counting |
| `runner/src/runner/faults.rs` | answer entries; a dispatch the core substitutes after the bindings check |
| `host/apple/src/executor_core.rs` | that substitution, after `bindings` and `timeout_refusal` |
| `host/web-js/admission.js`, `host/web/module-glue.js` | the answer after `admitsNetwork`; `fetchEarly` declines matched URLs |
| `host/linux/src/app.rs` | `EXACT_LAUNCH`; picture mode; the report; scale refusal |
| `host/linux` (a picture module) | D1a: activation, the data and motion fixed points, images, the busy walk, publishable |
| `motion/src/engine.rs` | a query for "an infinite animation is running" |
| `runner/src/runner.rs` | a public read of the failed-resource table |
| `host/linux/src/paint.rs` | the `flat` record at the refusal sites |
| `host/linux/src/text/catalog.rs` | the pinned set, no system scan, the fallback order |
| `scripts/fixtures/fonts/` | DejaVu Serif Book and Sans Mono Book subsets |
| `scripts/exact.mjs` | `render`, with the hard deadline |
| `apps/caltrain/pictures.test.contract` | the acceptance test |

## 7. The admission this needs

Proposed `rules/DEFERRED.md` entry, for slices 1 and 2 only:

> **Expanded (Charlie, <date>):** fixed fetch answers in tests, and a local
> picture of a URL under a state from the Linux CPU painter (LLP 1112 §3–§4).
> Consumer: Caltrain's home screen with the sky off. Unblocks reproducible
> screenshots and fixture-driven tests. Take: below. Still out: build and
> per-request pictures, the anonymous mode, PDF, paged media, a store snapshot.

Candidate takes, each reversing a recorded ruling: the JS render option
(Charlie, 2026-09-28: keep both, Rust primary, `llp/1071-exact2-web-target.rfc.md:526-532`),
or LLP 1069.009's development ring. Or a waiver.

## 8. Not in this RFC

Golden-image comparison as a test step (`rules/DEFERRED.md` refuses behavior
diff). Pictures from Apple or Windows. Video or animated pictures. Pictures as
a runtime feature. Request-chosen sizes, schemes or locales on a public path.
A store snapshot or database fixture. `http(s):` images in the Linux loader.

## 9. Open questions for Charlie

- **Q1 — The consumer.** Caltrain with the sky off, as §4 proposes, or a card
  route added to Caltrain so a picture needs no step?
- **Q2 — The take.** One of §7's, or a waiver.
- **Q3 — Emoji and scripts beyond Latin.** Grow the pinned set (Noto Color
  Emoji is about 10 MB), or require an app to declare its faces?
- **Q4 — `exact new` writes a `linux/` crate,** so an app outside the repo can
  render.
- **Q5 — `aria-busy` for pages.** Should the render host's page settlement take
  D1a (b)5's busy check? That is LLP 1048's call.

## 10. Review dispositions (r2)

r1's dispositions are recorded in the review files. Round 2: Astra (max) and
Grok 4.7 (xhigh), both NOT READY.

| Concern (who) | What r3 did |
|---|---|
| D1a is not a procedure: no activation, no fixed point, not repeated after images (Astra N1, BLOCKING; Grok BLOCKING) | D1a (a)–(e), each step citing the code it reuses |
| A seek moves infinite animations; the capture moves the session; ms vs s (both) | (c) states both and works in ms; a later step sees the sought time |
| `sync_surfaces`, `frame` while waiting, boolean `accessibilityBusy`, `land_then` runs a queued `next` (Grok) | (a), (b)1–5 |
| Settled is not publishable; `head.status` lost (Astra N3) | the publishable predicate; `status` in the report |
| Visibility differs from the painter's (Astra N4; Grok minor) | D2: refusal recorded inside the paint walk |
| The consumer fails D2: the sky is a GPU canvas (Grok) | §4: the sky toggled off by `tap sky-toggle`; the sky-on picture is the negative case |
| Slice 1 has no insertion point; `fetchEarly` sends the GET; cap, query, duplicates (Grok) | §3 names the three sites, the guard, 64 MiB, the whole URL, duplicates as errors |
| Omitted epoch, seed and `prefer` mean three launches; sibling locale order (both) | D9's table; `LAUNCH_MEDIA`; `LC_ALL`, `LC_MESSAGES`, `LANG` as `zone.rs` does |
| Production fixture answers contradict LLP 1103 D3 (Astra N10) | `answers` is development only; production refuses it |
| `exact render` can hang; 500 vs 503 (Grok) | D11: `exact render` enforces the hard deadline and exits 124; the server's codes are direction |
| URL-to-file collisions; a built file served for another query; asset digest; server time in keys (Astra N5–N6; Grok) | §5 (direction), each listed |
| Memory is not bounded in aggregate (Astra N7) | §5 (direction); D4 keeps only the size and scale refusal |
| Private-address refusal at the wrong layer and without ranges (both) | §5 (direction), with ranges and every hop |
| Build order, artifact handoff, `--render js` (Astra N9) | §5 (direction) |
| D9's example had `answers` in anonymous mode; the network-image path; historical timing; line drift (both, MINOR) | D9's example is development; `data:` URLs; timing marked historical; citations corrected |
| Implementer, take, apparatus approval (both) | Unchanged in substance: the header and §7 say so |
