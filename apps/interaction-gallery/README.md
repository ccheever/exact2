# Still · Interaction Gallery

The Photos, Arrange and Read consumers for [LLP 1041 §8.5](../../llp/1041-graceful-overload.rfc.md).
One Contract shell, one logical data owner, six local photographic illustrations.
Arrange and Read now use the shared viewport collection over complete records.
Photos retains explicit manual pages. Read authors a header-only numeric-height
binding; gesture recognition and hold delivery belong to the shared host adapters.
Photos authors fit/2× controls and a paired transform binding. The Web adapter
supports primary-pointer dragging; native photo delivery, animated reorder with
edge scrolling, and sheet/inner-scroll ownership transfer remain separate work.

## What works

- **Photos:** a wrapping grid, full-image reading surface, previous/next, deletion,
  and return to the current page of the selected stable identity. A moved or
  off-page source is located in the current order; a deleted source closes the
  viewer without resurrecting it. Fit/2× and reset are synchronous local actions;
  an authored drag surface targets the contained image's transform wrapper.
  On the web, drag that surface to pan, including while its spring is running.
  Changing Fit/2× during a drag updates the destination used on release.
- **Arrange:** pick up, preview earlier/later or before another card, Place, and
  Cancel. Preview never changes order. Place consumes an interaction token once;
  stale callbacks are inert. Concurrent insertion preserves the destination ID;
  removing the dragged record or its destination cancels the pending move.
- **Read:** 180 / 360 / 640px authored border-box heights and a real inner
  scrollport, using the same selected image and records. A dedicated non-button
  header handle targets the panel's authored `id="reading-sheet"` through
  `heightDragFor`; only that handle has `touch-action: none`. Buttons remain
  keyboard alternatives outside the handle, and the inner List keeps scrolling.
  The panel has `min-height: 0` and `max-height: 100%`, so the available stage
  constrains its displayed height, including padding.
- **All modes:** 100 / 1,000 / 25,000 records,
  distinct bounded insertion, deletion, reset, stable IDs, a 512-character scratch
  input, and responsive wrapping. Count/reset intentionally discard fixture edits.
- **Rendering controls:** Arrange and Read default to **Windowed**, supplying
  all logical records to the shared collection. **Manual 12** supplies one page;
  **Eager all** supplies and mounts the entire collection for diagnosis. The UI
  displays supplied and logical counts separately. Photos always supplies twelve
  or fewer records; its responsive grid is not claimed to be virtualized.

Ordering holds at most 25,000 compact IDs. A separate `galleryRows(revision, page,
full)` resource caches row values by structural revision and projection. Draft,
selection, preview, local sheet-height and other metadata changes neither
regenerate those values nor key all records again. A mutation returns only small metadata
and the selected record. Insert/delete/reset/committed reorder invalidate the
rows; generating and validating the changed full list still costs O(N). Ordinary
identity searches and earlier/later model actions can also scan the ID order.
There is no per-pointer-sample app action in this slice.

One Contract-local `sheetPx`, initially 360, is the authoritative authored target
for the panel and its selected button. `chooseSheet` synchronously selects one of
180/360/640. `heightrelease(height, velocity)` calls `snapSheet`, which chooses the
nearest stop to `height + velocity * 0.15`; the midpoint thresholds are 270 and
500, with ties choosing the taller stop. Velocity is displayed height units per
second, positive toward a taller sheet. Reset/count selection synchronously
restores 360; mode changes, selection and later data answers do not overwrite it.
The data model and metadata schema no longer contain `sheet` or `sheetHeight`,
and `galleryAction("sheet", ...)` is removed.

The host must apply the final pointer sample, dispatch this synchronous release
action and apply its resulting target while the hold is active, then end that
hold once with release velocity. Cancellation sends no snap action. The declared
transition is `height spring(300, 30, 1)`; projection changes the actual nested
List port on every presentation sample, without writing sampled height back into
`sheetPx`. The header's binding uses an authored IDREF, never `testId` inference.

Photos appends one bounded `viewer: list<ViewerState>` to gallery metadata. It is
empty when closed and contains one `{viewerToken, photo, naturalWidth,
naturalHeight}` while open. The existing eight-field Photo records and full-row
resource are unchanged. `each viewer in gallery.viewer key=viewer.viewerToken`
owns a child component whose `panX=0`, `panY=0`, `zoom=1` and geometry reset only
when that accepted lifetime changes. Open/Previous/Next and selecting a different
open photo reserve a new token before replacement; same-item selection, typing,
page and rank changes preserve the child. A successful lift closes the viewer;
an invalid ID or exhausted token refuses without replacing it.

The transformed wrapper fills its direct clip, with zero border, padding,
margin and positional displacement. Its centered origin, contained image and
transparent non-button descendant handle share that center. The handle declares
`transformDragFor="viewer-transform"`, `transformgeometry` and `transformrelease`;
controls remain outside the wrapper. The six verified source dimensions are
1448×1086, independent of decoded thumbnail resolution or display density.

`transformgeometry(boxWidth, boxHeight, portWidth, portHeight)` updates local
geometry and reclamps pan. For source dimensions `nw,nh`, the fit is
`min(boxWidth/nw, boxHeight/nh)`; each pan bound is
`max(0, (naturalSide*fit*zoom - portSide)/2)`. This accounts for letterboxing.
Zero/unavailable geometry keeps pan centered and cannot admit a physical hold.
Equal geometry makes no state writes. A changed geometry cancels the current
gesture in the host before new feedback; seamless resize rebasing is not claimed.

Fit and 2× write the latest authored zoom synchronously and clamp pan using that
new zoom. Reset centers and fits. `transformrelease(x,y,scale,vx,vy,vscale)` writes
only pan, clamped against the **current authored zoom**, never the caught scale.
The host validates the complete terminal tuple, applies the final paired sample,
dispatches this action while both holds are live, then ends each surviving owned
token once. Velocities go to the shared spring; the app does not project another
zoom target or issue a data request. Cancellation dispatches no release action.
Both declarations use `spring(300, 30, 1)`; there is no app timer or frame loop.

Each windowed row owns its spacing; the shared list receives the actual nested
scrollport and measured row heights. The sheet changes that port's height.
End-follow is explicit. Mounted views are bounded by viewport/overscan/pins;
the full source records and key metadata remain O(N). The eager diagnostic is
deliberately O(N) in views too. None of these choices establishes a flat process
or decoded-image memory bound.

Keyboard alternatives use semantic buttons. macOS/web declare Command/Control
1/2/3 for Photos/Arrange/Read; Escape closes the viewer or cancels a move;
Command/Control `[` / `]` switch photos; `,` / `.` move a preview earlier/later;
`P` places it. Pickup, deletion, page controls and sheet stops are ordinary
focusable buttons. Linux's current host lacks full button traversal/modifier
shortcut routing; app declarations alone do not fix or verify that host gap.
Windowed Place/Cancel focuses the persistent Arrange mode control. Manual/eager
actions retain the picked-up item's focus behavior. Seeking and focusing an
unmounted identity is not implemented; no proxy or extra row pin conceals that gap.

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
Rendering: `render-windowed`, `render-manual`, `render-eager`, `supplied-count`.
Viewer: `close-viewer`, `previous-photo`, `next-photo`, `delete-photo`,
`viewer-zoom-fit`, `viewer-zoom-detail`, `viewer-reset`, `viewer-clip`,
`viewer-transform`, `viewer-handle`.
Arrange: `reorder-scroll`, `lift-photo-00000`, `earlier`, `later`,
`before-photo-00015` (when mounted, or manual page 2),
`place`, `cancel`, `delete-photo-00000`. Read: `sheet-peek`, `sheet-read`,
`sheet-full`, `sheet-handle`, `sheet-scroll`, `note-photo-00000`.

## Shared hooks needed next

Shared height hold adapters are integrated and validated separately from this
app-only declaration. This app binds only the sheet header and synchronous snap
action; it does not claim physical pointer delivery from the app tests alone.
The photo binding has separate geometry/pair delivery in the shared host; app
tests alone do not prove physical drag, pinch or shared-element transitions.
The Web delivery now passes 22 checks against the real gallery Wasm in an
ordinary-clock browser: actual CDP mouse input, Fit activated with Space while
held, typing, contain bounds, resize/ancestor-scroll cancellation and keyed
source replacement. Focus for Space was placed with DOM `focus()`; a separate
single-property takeover case uses the real ABI directly. These are correctness
checks, not a latency, native parity or physical display result. Frozen sources,
raw exploratory failures and the final replay are in
`target/photo-gallery-browser/`; LLP 1041 §8.13 records the scope.
Continuous reorder still needs shared host hooks. Its existing `galleryAction`
endpoints provide preview and token-checked commit/cancel; they must receive
logical outcomes from the shared adapters.

1. Continuous begin/update/end/cancel delivery, with one current interaction token,
   cancellation on navigation/deletion, and no per-sample durable order commits.
   A takeover must sample current presentation position **and velocity**.
2. Stable-ID geometry lookup for photo return after reflow, scrolling, recycling or
   mutation; defined absent-source behavior. Pin or snapshot only the needed visual
   with explicit release and byte accounting. A logical page return here supplies
   identity, not a measured return rectangle.
3. Reorder presentation preview, nearby-card springs, bounded drag retention,
   geometry updates during resize, and edge auto-scroll in both directions.
   The existing model already separates proposed placement from committed order.
4. Sheet/inner-scroll ownership transfer at boundaries, retaining position and
   appropriate velocity through reversal and an interrupted settling spring.
   The initial header-only binding deliberately leaves this transfer out.
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

The photo app slice passes **41 scoped Rust tests** against the frozen common
binding/parser overlay: 18 model/resource, seven photo, eight retained runtime
and eight retained collection tests. Twelve tests are new, covering model
token/refusal, exact metadata/PNG dimensions, real-Contract keyed lifetime,
contained-pan math and pre-end paired-Engine targets. The old-app/new-common
baseline produced 14 failures (including two updated existing tests) first.
Strict data-crate all-target Clippy, scoped formatting/diff and source caps pass.
Sources and raw logs are preserved in `target/photo-app-validation` in the
private `exact2-gallery-photo-transform` validation tree.
These tests synthesize typed geometry/release events; host incarnation checks,
physical recognition, terminal velocity delivery and stale callback rejection
before clock movement need their own adapter tests. No physical pinch,
shared-element return, FPS or latency result is claimed by this app slice.

The collection increment passes **24 scoped Rust tests**: twelve model/resource
tests, six retained manual/focus regressions, and six new real-Contract collection
tests. The new tests failed against the manual foundation first. They cover
25,000 supplied records, the final identity, width/variable-height measurement,
the sheet's actual nested port, insertion anchor/offset preservation, unchanged
row allocation and zero record keying for metadata actions, commit/cancel, and
manual/eager controls. Scoped four-package Clippy with `-D warnings`, Rust
formatting and diff checks pass. No framework source or root manifest changes
belong to this app slice.

Optimized browser, actual AppKit and **actual Ubuntu 24.04.4 ARM64** software
presenter drives pass. Each reaches the last of 25,000 records, previews and
commits moving the first identity before the last without changing order during
preview, preserves typing, changes the nested sheet height, and retains manual
and eager controls. Eager was driven at 100 records, not 25,000. Browser runs
use 1180×860 and 420×860; AppKit and Linux resize during the drive from the former
to the latter. AppKit reports `NSWindow.setContentSize`; Linux reports actual
`Presenter.resize + frame`. No focus refusal or browser exception was recorded.

Artifacts are under
[`target/interaction-gallery-windowed/`](../../target/interaction-gallery-windowed/):

- `web-1180/report.json`, `web-420/report.json`: optimized headless Chromium.
- `macos-1180/report.json`: AppKit, including raw resize verification.
- `linux-actual/exact2-gallery-windowed-linux/evidence/report.json`: actual Ubuntu
  CPU painting with `/usr/share/fonts`, DejaVu Sans; copied executable beside it.
- Each report has raw command times, geometry and PNGs beside it. Captured
  windowed phases mounted 2–16 wrappers and 69–147 live nodes while supplying
  25,000 records. These are sampled functional observations, not a memory bound.

At 420 pixels wide, the actual nested Read port grows from Peek to Full:
browser 74.39→393.11 points, AppKit 74.10→392.18, Linux 76.30→396.94.
Inspected browser/Linux screenshots show readable rows and local photos at the
true end. Fast AppKit captures can precede asynchronous image decoding; retained
follow-ups allow 2.5 seconds and show the photos. This is a readiness observation,
not measured image-load latency. An initial temporary-driver coverage assertion
used view-ID order instead of screen Y order; its failure artifacts remain
preserved as `driver-unsorted-layout-failure.*`. Sorting observed boxes fixed the
driver; no app/framework change was needed.

Exact driven artifact SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| Optimized `app.wasm` | `fe9a0ad2843abf1ef64ce7104674c7868f9a4b6a9f529ead5a83d4ef4921a27c` |
| AppKit `ExactMac` | `74497fa433bf8b1367c89cb16040cab2a2cd2f8ab171936ee462ce3f2199a93a` |
| Ubuntu `interaction-gallery-linux` | `4acdf510023aef945adb1573c5062927661531a576ada59c49adb39cc93612b2` |

These builds use the current shared checkout, including other workers' host
changes, not an isolated release snapshot. Raw command round-trip time includes
driver queries/frames; it is not input-to-presentation latency. The Mac's JetKVM
display is 60 Hz. Linux here is headless software presentation, not the later
VKMS display drive. No physical 120 Hz, gesture-completion, flat-memory or
performance A/B claim is made.

### Earlier foundation evidence

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
logs are retained beside the screenshots. That earlier check was not an AppKit
or actual-Linux sweep; the collection increment's native results are recorded
above. Full native keyboard behavior, continuous gestures and performance
evidence remain separate work. Framework sources were still being
edited during this isolated check; it is not a release binary or performance baseline.
No physical FPS, 120 Hz presentation, touch arbitration, or performance A/B claim.
