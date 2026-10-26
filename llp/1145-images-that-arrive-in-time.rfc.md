# LLP 1145: Images that arrive in time

**Type:** RFC
**Status:** Draft r3, 2026-10-26. r1 was reviewed blind by GPT-6 Astra (max), Claude Fable 5.1 and Grok 4.7 (xhigh) (`llp/reviews/rfc-2026-10-26-1145-r1.{astra,fable,grok}.md`), r2 by Astra and Fable (`…-r2.{astra,fable}.md`; Grok's run returned only its opening line, twice); Muse was not available. Changes are in §R.
**Systems:** the Apple raster loader (`host/apple/Sources/ExactKit/RasterLoader.swift`, `RasterInput.swift`) and its diagnostics; the JS web target's `<img>` (`host/web-js/src/rows.rs`); `list`'s reach edges (the runner's collection window, `host/web-js/list.js`); LLP 1144's fling report
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-26
**Related:** LLP 1144 (the fling report, the `image` phase); LLP 1115 (no tells); LLP 1011 (`load` carries no payload); `issues/20261022-reach-lead-is-fixed-at-one-screen.md`

## R. Review, and what r2 changed

| Change | Raised by | Where |
|---|---|---|
| A remote image's wait is its download, which starts before any raster-core demand exists, in source-id order with no priority. Re-ranking the decode queue moved little. Priority is now the loader's, per view, and orders downloads as well as decodes. | all three | D1 |
| Direction comes from each scroller's own movement, which the loader already observes on iOS and macOS, not from the iOS scroll pump. | Grok | D1 |
| Measure first: the timings land before the ordering change, and are read on Excord. | Fable, Astra | D2, §4 |
| An image that sat on screen blank and then scrolled away is gone from `state` before anyone reads it. The loader keeps counts across retirements: images seen before their pixels, how long, and whether the wait was the download or the decode. | Fable, Astra | D2 |
| The JS target already emits `decoding="async"`, built ahead of review. The `width`/`height` attributes are dropped: CSS already reserves a box both of whose sizes are written, and attributes cannot supply the picture's ratio when only one is. | all three | D3 |
| The web screenshot already waits for images; r1's "an empty box a person never sees" was wrong. `clock settle` stays the app's clock alone (LLP 1054.000 R9). | Fable | §1 |
| A recipe cannot tell a cached image from a slow one, because `load` carries no payload. The fade is an open question with two answers, not a promise. | Fable | Open question 1 |
| `reach-margin` cannot by itself remove a fling's stall at the edge of loaded rows: no page fetched at that moment arrives in time. | Fable | D4 |

Second round (r2 → r3):

| Change | Raised by | Where |
|---|---|---|
| "Download or decode" cannot say why an image was late. Each image is stamped at every stage boundary (asked, admitted, header read after its bytes, decoded, delivered), and a late image is counted by the stage it was waiting on: admission, download, decode, or delivery on the main thread. Its lead, from being asked to first showing, is kept beside it. | Astra, Fable | D2 |
| Blank time is sampled on the frame clock, by the frame sampler, not on the loader's wakes, which during a fling are the downloads themselves. | Fable | D2 |
| D1 is gated on D2. If late images were asked in time and queued behind others, the ordering is built. If they were asked too late, the lever is fetching bytes when a page of data arrives. | Fable | D1, §4 |
| The loader observes scrolling only while something is parked. Direction comes from the presenter's signed per-list velocity, which both Apple hosts already report and the runner already uses to order its rows. | Fable | D1 |

## 1. What happens today

On Apple, an image starts loading when its view is mounted, not when it shows. The
runner's collection window mounts rows a viewport before and after what shows, plus
a quarter second of travel, so rows about a screen ahead are mounted in time. Their
images then go through three stages:

1. **Admission and download.** `inspectOne` starts remote fetches in source-id order
   on URLSession, with no priority.
2. **Decode.** A raster-core demand exists only once the header is read. It is
   `Visible` or `Overscan`, fixed at request time.
3. **Delivery** to the layer, on the main thread.

Under the memory budget, a request off screen is parked and waits until it shows.
Nothing records when each stage finished, and an image whose row retires leaves no
trace.

## 2. Decisions

### D1. The loader ranks every image, if D2 says it is the lever

Built only if D2 shows late images that were asked in time and then waited behind
others. If D2 shows them asked too late, ordering cannot help: no rank creates lead,
and the lever is fetching an image's bytes when the page of data that names it
arrives (§3).

The loader then keeps a rank per view: on screen, then ahead of its list's travel,
then behind, each by distance from the viewport. The direction is the presenter's
signed velocity for that list, which both Apple hosts already report. At rest the
nearer image wins. The rank orders:

- **Admission and downloads.** Sources waiting on the admission cap go best-ranked
  first, and each fetch's `URLSessionTask.priority` follows its rank.
- **Decodes.** A view that crosses onto the screen is re-requested, which the core
  already supports by replacing the demand for the same view and key.
- **Memory.** When the budget is short, rows behind the travel are parked before
  rows ahead.

### D2. When an image arrived, and why it was late

Each image is stamped as it passes each stage: asked for, admitted, its header read
(for a remote image, its bytes have arrived), decoded, delivered to its layer. On
each presented frame the frame sampler notes the images whose box shows with no
pixels yet. An image that showed blank is counted when its pixels land or its row
retires, so the images a fling passed survive their rows. For each one the loader
keeps:

- how long it showed blank;
- the stage it was waiting on: admission, download, decode, or delivery on the main
  thread;
- its lead, from being asked for to first showing.

`state.raster` carries the late count, blank p50/p95, lead p50 and p5, and the
counts by stage (`lateImages`), and each live image's stamps (`timing`). LLP 1144's
fling report prints them beside its frames. Presentation after delivery (Core
Animation's commit) is not measured. On the web the stamps are unknown until
resource timing and `img.decode()` are read, which is not built here.

### D3. The web target's `<img>`

Every `<img>` the JS target emits says `decoding="async"`, so a picture that arrives
during a scroll is decoded off the page's thread. This is built. No
`loading="lazy"`: the collection window already decides what is mounted, and lazy
loading would undo its lead.

### D4. A list's reach lead is the author's

`list` takes `reach-margin` (a length or a percentage of the viewport; one value, or
two for before and after; default one viewport, as now). It sets how far before an
edge `reachstart` and `reachend` fire. The runner owns the collection window, so the
native hosts need nothing; the JS target's `list.js` mirrors it. It moves a page's
fetch earlier. It does not stop a fling that outruns the loaded rows from stalling
at their edge: that would need the list to keep an estimated extent past them, a
change of its own.

## 3. Not proposed

- **`srcset` and `sizes`**: a schema addition with a native story; its own decision.
- **Fetching bytes for rows the data window holds but has not mounted.** That is the
  next lever if D2 shows downloads still late after D1. It is decided on D2's
  numbers.
- **JPEG, WebP and GIF on Linux and Windows**: already in `QUEUE.md`.

## 4. Plan

D2 (Apple), D3 and D4 are built. LLP 1144's report runs on Excord, with cold and
warm caches. D1, or fetching bytes at page arrival, follows from what it shows. D4's
conformance case is in `QUEUE.md`.

## Open questions

1. **A soft arrival.** Neither UIKit's image view nor a browser fades an image in,
   so a host default fade would be a tell (LLP 1115). An author can fade today with
   `opacity`, a `transition` and `load=`. But `load` carries nothing, so the app
   cannot tell a slow arrival from a cached one, and every recycled row fades. There
   are two ways to make it right:
   - **(a)** `load` carries the ms from mount to pixels, so the app fades only a
     slow arrival.
   - **(b)** An opt-in style row (`-exact-image-reveal: fade`) that the hosts run on
     the agent's clock, only for a slow arrival, and never under reduced motion.

   (b) is less for an author to get right; (a) is a smaller change to the platform.
