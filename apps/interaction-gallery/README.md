# Still · Interaction Gallery

The Photos, Arrange and Read consumers of the shared viewport collection and its
explicit height and transform bindings.
One Contract shell, one logical data owner, six local photographic illustrations.
Arrange and Read now use the shared viewport collection over complete records.
Photos retains explicit manual pages. Read authors a header-only numeric-height
binding; gesture recognition and hold delivery belong to the shared host adapters.
Photos authors fit/2× controls and a paired transform binding. Web, AppKit and
Linux support primary-pointer dragging; animated reorder with edge scrolling
and sheet/inner-scroll ownership transfer remain separate work.

## What works

- **Photos:** a wrapping grid, full-image reading surface, previous/next, deletion,
  and return to the current page of the selected stable identity. A moved or
  off-page source is located in the current order; a deleted source closes the
  viewer without resurrecting it. Fit/2× and reset are synchronous local actions;
  an authored drag surface targets the contained image's transform wrapper.
  On Web, AppKit and Linux, drag that surface to pan, including while its spring is running.
  Changing Fit/2× during a drag updates the destination used on release.
- **Arrange:** pick up, preview earlier/later or before another card, Place, and
  Cancel. Manual preview never changes order. Place consumes an interaction token
  once; stale callbacks are inert. Concurrent insertion preserves the destination
  ID; removing the dragged record or its destination cancels the pending move.
  Windowed rows also author a separate non-button grip with
  `reorderFor="arrange-list"` and `touch-action="none"`. The grip is disabled
  during a manual move and absent in Manual/Eager modes. Continuous physical
  delivery and preview remain shared adapter work, not an app-test claim.
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

The windowed List owns `reorderdrop(item: string, before: option<string>)`.
Its synchronous action calls `galleryReorder(item, before, gallery.revision)`;
`none` means the end, while `some("")` is an invalid identity, not an end alias.
This single terminal operation validates the current revision, manual-move gate,
source and destination before changing order. Common physical delivery must also
certify that its preview still belongs to this collection and source; synthesized
app events alone do not establish stale-pointer or incarnation safety.

An accepted move advances a checked structural revision once. Reset, insertion,
deletion and changed manual Place also reserve that revision before consuming
interaction/record tokens or mutating order. Revision exhaustion refuses; it never
saturates or wraps. Self, adjacent and already-at-end placements are unchanged,
including at the maximum revision. An unchanged manual Place may still finish
its manual interaction without changing the order or revision.

Ordinary atomic refusals return the existing 26-field metadata with a transient
`Move refused: …` notice. They do not alter the model or either cached row
allocation, and do not poison the Runner. Malformed argument types remain a data
error. Accepted and unchanged snapshots retain the eight-field Photo wire; there
is no extra result slot, preview order, or per-pointer data query. Terminal
identity lookup and rebuilding changed rows remain O(N).

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
transition is `height -exact-spring(300, 30, 1)`; projection changes the actual nested
List port on every presentation sample, without writing sampled height back into
`sheetPx`. The header's binding uses an authored IDREF, never `testId` inference.

Photos appends one bounded `viewer: ViewerState[]` to gallery metadata. It is
empty when closed and contains one `{viewerToken, photo, naturalWidth,
naturalHeight}` while open. The existing eight-field Photo records and full-row
resource are unchanged. `for viewer in gallery.viewer key=viewer.viewerToken`
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
the zoom as the released scale held between Fit (1) and 4×, and pan clamped
against that new zoom (a pinch adopts its scale; a pan releases the scale it
caught). Fit, 2× and Center stay the controls that need no gesture.
The host validates the complete terminal tuple, applies the final paired sample,
dispatches this action while both holds are live, then ends each surviving owned
token once. Velocities go to the shared spring; the app does not project another
zoom target or issue a data request. Cancellation dispatches no release action.
Both declarations use `-exact-spring(300, 30, 1)`; there is no app timer or frame loop.

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

The app is four root workspace members:

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
cargo build --profile host-dev -p interaction-gallery-linux
```

The pure model can be tested before workspace integration:

```sh
rustc --edition 2021 --test apps/interaction-gallery/data/src/model.rs \
  -o /tmp/interaction-gallery-model-tests
/tmp/interaction-gallery-model-tests
```

Agent IDs: `count-100`, `count-1000`, `count-25000`, `mode-photos`,
`mode-reorder`, `mode-sheet`, `previous-page`, `next-page`, `insert`, `reset`,
`gallery-input`; initially `open-photo-00000` through `open-photo-00011`.
Rendering: `render-windowed`, `render-manual`, `render-eager`, `supplied-count`.
Viewer: `close-viewer`, `previous-photo`, `next-photo`, `delete-photo`,
`viewer-zoom-fit`, `viewer-zoom-detail`, `viewer-reset`, `viewer-clip`,
`viewer-transform`, `viewer-handle`.
Arrange: `reorder-scroll`, `arrange-grip-photo-00000`, `lift-photo-00000`, `earlier`, `later`,
`before-photo-00015` (when mounted, or manual page 2),
`place`, `cancel`, `delete-photo-00000`. Read: `sheet-peek`, `sheet-read`,
`sheet-full`, `sheet-handle`, `sheet-scroll`, `note-photo-00000`.

## Shared hooks needed next

Shared height hold adapters are integrated and validated separately from this
app-only declaration. This app binds only the sheet header and synchronous snap
action; it does not claim physical pointer delivery from the app tests alone.
The photo binding has separate geometry/pair delivery in the shared host; app
tests alone do not prove physical drag, pinch or shared-element transitions.
The Web adapter now provides primary-pointer reorder, spring-aside preview,
edge scrolling and interrupted return/regrab through the shared collection
mechanism. The Linux adapter has matching Presenter/CPU coverage; its actual
Linux display drive and the Apple adapter remain follow-up work. The `galleryAction`
preview and token-checked Place/Cancel endpoints remain the manual controls;
only the final logical pointer outcome reaches synchronous `galleryReorder`.
There are no per-pointer app-data requests. Shared obligations and remaining
cross-host work include:

1. Continuous begin/update/end/cancel delivery, with one current interaction token,
   cancellation on navigation/deletion, and no per-sample durable order commits.
   A takeover must sample the current presentation. The initial reorder terminal
   policy permits positional continuity with a zero-velocity neighbor restart;
   it does not claim continuous neighbor velocity.
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
