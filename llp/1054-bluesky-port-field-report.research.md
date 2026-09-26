# LLP 1054: The Bluesky port field report

**Type:** Research
**Status:** Draft
**Systems:** All; chiefly building an app outside the repository (scripts, toolchains), the Contract language and compiler, the data seam (records, grants, native HTTP admission, re-asking), host parity (iOS lists, AppKit clipping and text), and the agent driver
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-26
**Related:** LLP 1049 (the Crew port report, the first of these); LLP 1054.000 (the proposals this report produced); LLP 1036.001 (apps outside the repository); LLP 1027.000.000 (the date as a host fact, implemented in its minimal form by this port); LLP 1016 (requests and settlement); LLP 1041 §8.4 (native admission); LLP 1012 (the agent API); LLP 1017.000 (Contract v1.1); LLP 1038 (router)

## Summary

On 2026-09-26 an agent built a Bluesky client on exact2 overnight, as an
app outside the repository (`github.com/ccheever/bluesky-exact2`, consuming
`../exact2` by path). It has one Contract source (~2,150 lines in six files)
and a Rust data crate (~4,150 lines) speaking the AT Protocol, and it runs
on the web, macOS and the iOS simulator. It covers feeds, threads, profiles,
search, notifications, chat, compose, optimistic likes and follows, and a
desktop layout. The agent kept a diary (`DIARY.md` in that repository) of
every problem; this document catalogs it.

Six gaps were closed in exact2 during the port and merged at `f115a0e6`:
the date as a host fact, pull to refresh, WebP sizes on Apple, link
activation, app icons, and symbol roles (§1). Thirty findings remain open
(§2–§6). Their proposed resolutions are LLP 1054.000.

The port also confirmed what works. The same source ran on three hosts
nearly first try. The compiler's diagnostics named every fix. The data
seam's "answer now or hand back one request" shape carried pagination,
multi-step requests, token refresh and an offline demo account with one
mechanism. Web boot to first paint was 70–110 ms, and a tab tap changed the
DOM in 6.4 ms (§7).

Unlike the Crew report, most findings here are about *authoring*: what the
language and the seam make an app writer repeat, guess or work around.

## Findings

Each finding has an id, the evidence from the port, what it cost, and
existing coverage. Severity: **S1** blocks or corrupts an app with no
workaround inside the app; **S2** has a workaround that costs code or
correctness; **S3** costs time once.

### 1. Closed during the port (merged `f115a0e6`)

| id | what | fix |
|---|---|---|
| X1 | No date: `now()` is elapsed time, so "5m ago" was impossible (the HTTP `Date` header is hidden cross-origin) | `exactTime` (`epochAtZero`, `utcOffset`), `Runner::set_time`, web glue and Apple `Session.tellTime()`; LLP 1027.000.000's minimal form. Linux not wired; no agent substitute epoch |
| X2 | No pull to refresh | `refresh` event (EventKind, Apple ABI kind 22) and `refreshing` prop; iOS `UIRefreshControl`. Web and AppKit have none |
| X3 | Apple never loaded a WebP over 256 KB (`image deferred: headerLimit`); the Bluesky CDN serves every image as WebP | the RIFF header answers the size when ImageIO's incremental source cannot |
| X4 | A link inside a `press` element fired both (web); an inline `href` to an app path did nothing (iOS) | a nested link is the innermost activation; app paths go to the navigation root |
| X5 | Apple builds had no app icon | from the manifest's first square `icons` entry ≥ 512 px |
| X6 | No heart, reply, share, ellipsis or fill roles | 21 symbol roles |
| X7 | The web shell outlined every hovered button (1 px, `currentColor`) | removed; a bare `<button>` has none. **Charlie to confirm** |

### 2. Starting an app outside the repository

- **O1 (S3) — No scaffold.** The documented template (Weird Castle) was not on
  disk; its `build.rs` had drifted from the in-repo apps' (`web_linked`,
  `web_rust_mode`). Five profiles, two patches, the toolchain file and the
  host crates' `lib.rs`/`build.rs` were copied by hand. ~30 minutes.
  Covered: LLP 1036.001 D2 (unassigned).
- **O2 (S3) — Prerequisites arrive one failure at a time.** In order: the
  pinned nightly with `rust-src`; `cargo fetch` in the app workspace (the
  build is `--offline`); `cargo +nightly fetch` of std's lockfile
  (`-Zbuild-std`); binaryen *exactly* 132 for `wasm-split` (133 refused;
  no download line; "brew install binaryen" on a machine without Homebrew);
  `DEVELOPER_DIR` when `xcode-select` names the Command Line Tools (the iOS
  build fails inside `ibex2`'s build script). Five builds to learn five
  prerequisites, each named well only when hit.
- **O3 (S2) — TypeScript on Apple needs Hermes by hand.** `apps/realworld`,
  the natural template for an HTTP app, is web-only; its Apple path needs a
  ~20-minute Hermes build. The port chose Rust instead. Covered: issue
  `ios-hermes-provisioned-by-hand`, LLP 1036.001 D5.
- **O4 (S3) — `metrics --app` reports n/a for every native row** ("has no
  web/src/bin/metrics.rs"); there is no template for that bin.
- **O5 (S2) — One `host/web/dist` per checkout, last build wins.** The five
  checks rebuilt Caltrain into it and the served Bluesky page silently
  became Caltrain's. After an agent web run, `serve.mjs` refused the
  directory ("run bun host/web/build.mjs first") until the next build.

### 3. Learning and writing Contract

- **L1 (S3) — No single vocabulary reference.** Tags live in
  `contract/lower/src/tags.rs`, built-ins in `plan/tables/format.json`,
  symbols in `kernel/tables/schema.json`, style names in LLP 1017.000 P9,
  collections in `collection.rs`, router verbs in LLP 1038 §3. Reading
  Messages end to end was the fastest way in.
- **L2 (S3) — Reserved words are not named as reserved.** A field `key` or
  an action `view` gives `syntax-expected-name … found key`.
- **L3 (S2) — A `button`'s label sits at its top.** `button` is a flex
  column, so `align-items: center` centres horizontally only and every
  fixed-height pill needed `justify-content: center` too. A bare HTML
  `<button>` centres its content on both axes, so this is arguably a
  deviation from "the web is the standard", not a style choice.
- **L4 (S3) — `clip-path: path()` takes only absolute `M/L/Q/C/Z`.** Real SVG
  (Bluesky's butterfly) is relative; converted with a script. Covered:
  issue `clip-path-subset-of-css`.
- **L5 (S2) — No list indexing, and `fn` cannot loop.** "The first image"
  needed the data crate to number ids and `each … when p.id == "0"`. Finding
  the viewer's own avatar in `list<Profile>` needed a whole data source.
- **L6 (S3) — `display` is refused on a virtualized list's root.** Keeping a
  list per home feed mounted needed a wrapper column to carry `display`.
- **L7 (S3) — `reachend` cannot take bound arguments.** See D4.
- **L8 (S2) — Record resources need a hand-written placeholder source each.**
  The bake asks every resource; a record whose first answer is `Later` needs
  `else emptyX()` and an `emptyX` source in the data crate. The diagnostic
  says so exactly; it is still one boilerplate source per shape (six here).
- **L9 (S3) — Compiler diagnostics are buried during builds.** Inside
  `build.mjs` a Contract error is a `build.rs` panic in ~200 lines of cargo
  JSON. The port used `contract build app.contract -o /tmp/x.plan` as its
  inner loop instead.
- **L10 (S2) — Plan size is invisible, and it is wasm size.** Every
  component use is inlined, and the plan is `include_bytes!`'d into the wasm
  (and served again as `app.plan`). `Row` (~100 nodes) was used six times
  and carried a thread-only `FocusPost`; splitting it and merging two screens
  took the plan from 1,546 nodes / 259 KB to 1,063 / 193 KB with no behaviour
  change. Nothing reports which component costs what.

### 4. The data seam

- **D1 (S2) — Rust sources build records by position, untyped.** A
  TypeScript source gets `app.contract.d.ts`; a Rust source gets nothing and
  writes `Value::record(vec![…])` in field order. A field added mid-shape
  silently shifts the rest; the runner's shape check catches it only at
  runtime. The port mirrored ~20 shapes by hand, with the order recorded in
  comments.
- **D2 (S1) — A source cannot say its answers changed.** The runner re-asks
  a resource only when an argument changes. When a `send` changes what a
  resource would answer (a pending message, a read badge), the app must also
  bump a state the resource takes as an unused argument. The port threads
  `rev` through eight resources and forgot it twice: sent messages did not
  appear, and a read chat kept its badge.
- **D3 (S2) — Re-asking a resource forgets its request in flight.** Any
  argument change during a load issues the same request again and drops the
  first reply (`forget request 2 … reply 2 dropped`). Two causes in this
  port: the date arriving just after boot changes a `clock` argument, and
  `reachend` firing on a short loading list. Workarounds: lists wait while
  `clock == 0`, and "load more" is guarded by a page count computed by an
  extra resource.
- **D4 (S2) — `reachend` cannot say which list or how far it got.** Its
  handler takes no bound arguments, so the guard in D3 needed a separate
  `loaded(homeFeeds, authorFeeds)` resource to read page counts off lists.
- **D5 (S1) — Independent reads beyond the budget are refused, not queued.**
  The native executor reserves each independent read's
  `max_response_bytes` against 32 MiB. At 8 MiB the fifth launch read was
  refused ("native executor admission limit reached") and the app showed a
  network error. The knob is a reservation, not a cap, and nothing says so
  outside `executor_core.rs`.
- **D6 (S3) — HTTP is ordered by default.** Overlap is opt-in per request
  with `.independent_http(bytes)`, found only in `runner/src/request.rs`.
  For a feed client almost every read wants it.
- **D7 (S2) — `net.fetch` grants are exact origins.** A Bluesky account lives
  on one of many PDS hosts (`*.host.bsky.network`). The port routes every
  signed-in call through the `bsky.social` entryway, which forwards.
- **D8 (S3) — Time needs an argument on every list.** With `exactTime`,
  relative times still re-render only through a minute-granular `clock`
  argument each list resource takes, plus a tick task.

### 5. Host parity

- **P1 (S2) — iOS: a list row's height ignores a child's negative margin.** A
  profile header row with `margin-top: -44` on a child measured 44 pt taller
  than its content on iOS (the web's was right). Found by dumping frames on
  both hosts; worked around with an absolutely positioned avatar.
- **P2 (S2) — AppKit: `overflow: hidden` with `border-radius` does not clip
  an image child to the corners** (the web and iOS do). Avatars came out
  square; worked around with a radius on each image.
- **P3 (S2) — AppKit: a `line-clamp: 1` text whose one word is wider than its
  box draws over its neighbour.** CSS clips it (line-clamp implies
  `overflow: hidden`), and the web does. Worked around with
  `white-space: nowrap; text-overflow: ellipsis; overflow: hidden`.
- **P4 (S3) — Pull to refresh exists on iOS only** (X2). A `refresh` handler
  is silently inert on the web and AppKit.

### 6. The agent driver

- **A1 (S2) — `clock settle` does not wait for images.** Every web and iOS
  screenshot showed grey placeholders; headless Chrome with a virtual-time
  budget showed the images loading. (iOS is also in QUEUE.)
- **A2 (S3) — `layout` takes an id, and ids are not stable** across runs of a
  live-data app. The port wrote a 25-line script over `agent.mjs` `open()`
  to find nodes by testId first.
- **A3 (S3) — AppKit screenshots omit SF Symbols**, and `screenshot … window`
  needs Screen Recording permission. Symbols were checked by `layout`'s
  `native.symbol` instead. Covered in part: issue
  `macos-offscreen-capture-can-be-transparent`.
- **A4 (S3) — A pull (or any phase) is `unsupported` on iOS**, so X2 was
  verified by building and reading only.
- **A5 (S3) — `clock <far future>` fails with `TimerFireLimit`** when a
  250 ms task is mounted. It is correct, but it made "show me this at a real
  date" impossible, which is X1's agent-substitute gap.

### 7. What worked, to keep

- One source, three hosts: native navigation (swipe-back, sheets), native
  symbols, a projected tab bar, safe areas, `light-dark()` dark mode, a
  desktop rail layout from `exactViewport`.
- Diagnostics with an id, a location and the fix, for every compiler error
  the port hit.
- The data seam's single loop (answer now, or one request; `parse` folds the
  reply in and asks again): pagination, handle → did → thread, token
  refresh, notifications → posts, and a `demo` account answering in-process
  through the same parse path.
- The runner's order (a `send`'s answer before the action's writes settle)
  made optimistic UI a matter of an overlay in the source.
- CSS fidelity, including where it bit: `border-style: solid` with two widths
  gave a 3 px top border (`medium`), exactly as a browser would.
- Numbers: wasm 392–469 KiB gzip; script → DOM 27–40 ms; first paint
  72–112 ms; tap → DOM 6.4 ms.

## Confidence

High for every S1/S2 finding: each was reproduced and worked around in the
port, with the workaround in the app's history. P1–P3 were measured by
frame dumps on two hosts. D5's budget arithmetic is from
`host/apple/src/executor_core.rs` (`BYTES`, `reservation`). L10's numbers
are from the compiler's own summary line. Lower for O2's "five builds": the
order depends on what a machine already has.
