# Still · Interaction Gallery

The Photos, Arrange and Read consumers for [LLP 1041 §8.5](../../llp/1041-graceful-overload.rfc.md).
One Contract shell, one logical data owner, six local photographic illustrations.
This is the visual and logical foundation for the three interactions. Continuous
zoom/drag, animated reorder with edge scrolling, and sheet/inner-scroll transfer
are **not connected**. The UI currently exposes the discrete actions described below.

## What works in the foundation

- **Photos:** a wrapping grid, full-image reading surface, previous/next, deletion,
  and return to the current page of the selected stable identity. A moved or
  off-page source is located in the current order; a deleted source closes the
  viewer without resurrecting it. There is no claimed zoom animation yet.
- **Arrange:** pick up, preview earlier/later or before another card, Place, and
  Cancel. Preview never changes order. Place consumes an interaction token once;
  stale callbacks are inert. Concurrent insertion preserves the destination ID;
  removing the dragged record or its destination cancels the pending move.
- **Read:** three discrete reading heights and a real inner scrollport, using the
  same selected image and records. Buttons select heights; they are keyboard
  alternatives and layout fixtures, not simulated direct manipulation.
- **All modes:** 100 / 1,000 / 25,000 records, explicit 12-record manual pages,
  distinct bounded insertion, deletion, reset, stable IDs, a 512-character scratch
  input, and responsive wrapping. Count/reset intentionally discard fixture edits.

Ordering holds at most 25,000 compact IDs. A snapshot returns at most twelve row
records and one selected record. Page changes do not regenerate image files.
This bounds the supplied UI records; it is **not viewport virtualization** and
does not establish a flat host decoded-image memory bound. No eager 25,000-view
mode is provided by this foundation. The shared collection integration comes next.

Keyboard alternatives use semantic buttons. macOS/web declare Command/Control
1/2/3 for Photos/Arrange/Read; Escape closes the viewer or cancels a move;
Command/Control `[` / `]` switch photos; `,` / `.` move a preview earlier/later;
`P` places it. Pickup, deletion, page controls and sheet stops are ordinary
focusable buttons. Linux's current host lacks full button traversal/modifier
shortcut routing; app declarations alone do not fix or verify that host gap.

## Build integration

The lead integrated these four root workspace members together:

```toml
"apps/interaction-gallery/data",
"apps/interaction-gallery/web",
"apps/interaction-gallery/apple",
"apps/interaction-gallery/linux",
```

Packages have the corresponding `interaction-gallery-` prefix. The app ID is
`com.exact.interaction-gallery`. App-local entrypoints follow Markdown Stress's
shared host templates, with no new executor, capability, or upstream dependency.
From the repository root:

```sh
export EXACT_UPDATE_TRUST=development
cargo test -p interaction-gallery-data
bun host/web/dev.mjs --app interaction-gallery
bun host/apple/build.mjs --app interaction-gallery --run
cargo build --release -p interaction-gallery-linux
```

The pure model can be tested before workspace integration:

```sh
rustc --edition 2021 --test apps/interaction-gallery/data/src/model.rs \
  -o /tmp/exact2-interaction-gallery-model-tests
/tmp/exact2-interaction-gallery-model-tests
```

Agent IDs: `count-100`, `count-1000`, `count-25000`, `mode-photos`,
`mode-reorder`, `mode-sheet`, `previous-page`, `next-page`, `insert`, `reset`,
`gallery-input`; initially `open-photo-00000` through `open-photo-00011`.
Viewer: `close-viewer`, `previous-photo`, `next-photo`, `delete-photo`.
Arrange: `lift-photo-00000`, `earlier`, `later`, `before-photo-00015` (page 2),
`place`, `cancel`, `delete-photo-00000`. Read: `sheet-peek`, `sheet-read`,
`sheet-full`, `sheet-scroll`, `note-photo-00000`.

## Shared hooks needed next

The motion engine already has `Engine::hold`, release velocity on `observe`,
interruption from current presentation, and `VelocityTracker`. Apple's horizontal
swipe bridge and the web's `swiperight` handler provide narrower existing paths;
they do not establish these three interactions. LLP 1013 explicitly defers
interactive/scrubbed transitions.

1. Continuous begin/update/end/cancel delivery, with one current interaction token,
   cancellation on navigation/deletion, and no per-sample durable order commits.
   A takeover must sample current presentation position **and velocity**; Apple's
   existing `drag_x` currently starts holds from the authored target plus delta.
2. Stable-ID geometry lookup for photo return after reflow, scrolling, recycling or
   mutation; defined absent-source behavior. Pin or snapshot only the needed visual
   with explicit release and byte accounting. A logical page return here supplies
   identity, not a measured return rectangle.
3. Reorder presentation preview, nearby-card springs, bounded drag retention,
   geometry updates during resize, and edge auto-scroll in both directions.
   The existing model already separates proposed placement from committed order.
4. Sheet/inner-scroll ownership transfer at boundaries, retaining position and
   appropriate velocity through reversal and an interrupted settling spring.
   The changing scrollport must drive the shared collection's window.
5. Presented-geometry hit testing, focus restoration, reduced-motion behavior,
   and keyboard parity on all hosts. Pointer cancellation and retained lifetimes
   need actual host tests; a logical token test cannot prove them.

These should use shared framework mechanisms. There is no app-private gesture
engine, portal, decoded image cache, timer or alternate presentation graph here.

## Raster provenance and bounds

All six images were generated specifically for this app using OpenAI's built-in
`image_gen` tool on 2026-09-17 UTC. They depict imagined locations. They are not
stock downloads, personal photographs, or attributed to a real photographer.
No third-party photograph license is being claimed. Each image was generated
separately and visually inspected; none is a crop of a contact sheet.

[`assets/provenance.json`](assets/provenance.json) records each original prompt,
scene, SHA-256, dimensions and exact file size. The original PNGs are bundled
unchanged. No runtime external dependency or image URL is used.

| Fixture | File bytes | Dimensions |
| --- | ---: | --- |
| North shore | 3,096,498 | 1448 × 1086 |
| Ochre dunes | 1,949,637 | 1448 × 1086 |
| Alpine water | 2,820,542 | 1448 × 1086 |
| Winter ridge | 1,692,865 | 1448 × 1086 |
| Glasshouse | 2,923,054 | 1448 × 1086 |
| Last light | 2,645,867 | 1448 × 1086 |

Total PNG bytes: **15,128,463 (14.43 MiB)**. One decoded RGBA copy of all six is
37,740,672 bytes (35.99 MiB). That arithmetic is not a measured process-memory
claim: hosts may hold multiple decoded/presentation copies. Every fixture record
maps deterministically onto this fixed pool. Assets are distinct; the larger
record counts intentionally repeat them to bound the bundle.

## Validation status

Before root workspace integration, an isolated temporary Cargo manifest compiled
this data crate against the checkout's real compiler, runner and kernel.
Ten model/argument-validation tests and four initial Contract/runner tests passed.
The tests exercise actual press dispatch and focus commands targeting mounted controls,
cross-page return, deletion, editing preservation, preview/commit/cancel, mode
changes and the bounded 25,000-record projection.

Actual web review subsequently found that focus resolves the authored `id`, while
the controls only carried `testId`. The initial test checked mounting but missed
that distinction. The strengthened tests failed before the fix. All focus targets
now carry an authored `id`; tests require that exact `PropId::Id`, uniqueness among
live nodes, an enabled button, and the matching post-commit command. Returning
across a photo page boundary and placing a card on another page are covered.
Cancelling a move after paging away now remounts the picked-up identity's page
before focus, without changing order. Ten model tests and **six runtime tests**
pass after this correction, as does scoped data-crate Clippy. Before/after test
logs are retained in `/tmp/exact2-gallery-evidence/focus-fix/`.

After rebuilding the optimized web app, the lead's actual headless-browser drive
passed: returning across pages revealed `photo-00011` intersecting the scrollport,
with **zero focus-refusal logs**. Photo deletion, reorder cancel/commit, sheet
height controls, draft preservation and the 25,000-record manual projection also
passed. Results and logs are in
[`target/interaction-gallery-web-review/report.json`](../../target/interaction-gallery-web-review/report.json),
beside `photos-wide.png`, `viewer-wide.png`, `arrange-wide.png` and `sheet-wide.png`.
The original `failure.json` and `failure.png` remain preserved. This verifies
discrete actions and browser focus/reveal, not continuous gestures, viewport
virtualization, or physical presentation performance.

After root integration, these normal `--locked --offline` workspace checks passed
for the initial foundation, before the focus correction:

```sh
cargo test --locked --offline -p interaction-gallery-data
cargo build --locked --offline -p interaction-gallery-web \
  -p interaction-gallery-apple -p interaction-gallery-linux
cargo build --locked --offline --target wasm32-unknown-unknown \
  --lib -p interaction-gallery-web
cargo clippy --locked --offline -p interaction-gallery-data \
  -p interaction-gallery-web -p interaction-gallery-apple \
  -p interaction-gallery-linux --all-targets -- -D warnings
```

These builds used `EXACT_UPDATE_TRUST=development`. The wasm command needs
`--lib`: the package's resident `dev` compiler binary is for the build machine.
The initial all-target wasm invocation failed on that native-only binary; the
web library build above passes. Rust formatting checks pass. All six asset hashes,
file lengths and PNG dimensions match provenance. Source files fit the existing
1,500-line cap; no new check is registered.

The real Linux CPU presenter was driven headlessly **on macOS**, with local fonts
and image decoding, at 1180×860 and 420×860. Screenshots and layout JSON are in
`/tmp/exact2-gallery-evidence/`: `photos-wide`, `viewer-wide`, `arrange-wide`,
`sheet-wide`, `photos-narrow`, `arrange-narrow`, `sheet-narrow`,
`sheet-full-narrow`, `sheet-peek-narrow`. All nine screenshots were inspected.
They show actual app output, not design mockups. The narrow grid wraps to one
column; the reading panel's changing height clips its own scrolling content.

The baked `target/debug/interaction-gallery-linux` executable also passed smoke
on macOS using CPU painting and Helvetica Neue (787 discovered faces). Its first
smoke capture, `baked-photos-wide.png`, still shows the colored placeholders:
the host's 500 ms image wait expired during this debug run. The agent-driven
follow-up, `baked-photos-settled.png`, displays all six photos. The driver allowed
six seconds and polled; this is a readiness check, not an image-load latency
measurement. `baked-agent.json` and `baked-agent.stderr.log` retain the exchange.
The app exits cleanly. This exposes a first-frame image-readiness limit; it is not
hidden by the functional smoke result. The isolated presenter captures above wait
up to five seconds for local decoding.

`evidence.json` records executable and source digests; workspace test/build/lint
logs are retained beside the screenshots. This is not an AppKit or actual-Linux
sweep. Those native app builds, native keyboard/focus behavior, continuous gestures and performance
evidence remain separate validation work. Framework sources were still being
edited during this isolated check; it is not a release binary or performance baseline.
No physical FPS, 120 Hz presentation, touch arbitration, or performance A/B claim.
