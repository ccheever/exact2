# LLP 1052: Jordan Walke on exact2 — notes from a conversation

**Type:** Notes
**Status:** Draft. Charlie's notes of a 2026-09-24 conversation with Jordan Walke, what was concluded from them that night, and what is still to ask him. It decides nothing.
**Systems:** Runner and the data seam (how Rust and TypeScript hold and share values), Motion and the hosts' gesture recognition, Kernel (hypothetical layout)
**Author:** Charlie Cheever (the notes); Claude (Opus 5.5) (the write-up)
**Date:** 2026-09-24 (the conversation); written up 2026-09-25
**Related:** LLP 1051 and LLP 1051.000 (hypothetical layout: the follow-up to the third point); LLP 1027.003 and LLP 1027.004 (value transfer measured; bounded answers); LLP 1002 D4 and D5, LLP 1035.001, LLP 1041 §8.5 (gestures); `rules/DEFERRED.md` §Motion

## The notes, as written that afternoon

> He told me about reimplementing the gesture system from ios and running on
> the main thread, and that it missed a few things.
>
> He told me about preflex or something which is like computing layouts on the
> main thread sort of hypothetically in order to do things like reparenting
> correctly.
>
> He brought up issues about garbage collection and rust type stuff being
> unable to evict stuff or being slow on allocation

## Charlie's fuller account, that evening

1. **Rust and garbage collection.** *"He was very focused on how Rust has
   trouble interoperating with garbage collected languages and it's hard to do
   on the web. Allocations can be expensive. Hard to know when to evict things.
   GC often better. Hard to interop with JS."* He asked how Rust and
   TypeScript store and share data, and what that substrate is. *"He seemed to
   think it will be the problem and bottleneck as apps scale and get more
   complicated."*
2. **Gestures.** *"He thought iOS handled gestures the best of any platform
   he'd seen and he spent a bunch of time reimplementing that system for other
   platforms like web."*
   - He found problems with it, *"like wanting to exclude touches below or
     above in certain subtrees when doing multiple gesture recognitions at
     once."*
   - *"He thought a DSL like Reanimated uses was not as good as being able to
     just write code on the main thread to handle anything."*
3. **Preflex.** *"Some system called Preflex or something that was kind of
   like Cheng Lou's Pretext thing but more general … it seemed like you could
   on the main thread, compute a hypothetical layout and then adjust your
   gestures or animations or whatever accordingly based on where things would
   be and how much space they would take up."*

## What was concluded that night

This is from sweeps of the code in a Claude session on 2026-09-24. It is
analysis, not Jordan's words. The three points share one theme: app code
reaching the UI synchronously, on the main thread, to get at its data,
gestures and geometry. exact2 says no in each area, and each no traces back to
something that went wrong in exact1.

- **Data: the design holds, but it was never measured at scale.**
  - Rust and TypeScript never share a heap. Every value crosses as a copy: JSON
    for TypeScript, canonical bytes for Rust modules and workers.
  - Rust values are immutable reference-counted trees, freed as soon as the
    last reference drops. JavaScript values belong to their engine's garbage
    collector.
  - LLP 1027.003 found that plain JSON beat immutable views, typed buffers and a
    shared heap. The result that mattered was granularity: at 100,000 rows an
    edit cost about 0.3 ms with a 100-row window, against 78 ms for the whole
    list. LLP 1027.004 made windowed answers the rule.
  - Where Jordan is right: data that is not windowed hits the limit early.
    Mounting 25,000 rows eagerly kept about 96 MiB alive for 2.1 MiB of input
    (LLP 1010). Whether the runner itself should use garbage collection was
    never studied, and wasm memory never shrinks.
  - Next: measure Messages at 100,000 rows on a real phone — crossing cost, JS
    and wasm memory together, and whether wasm memory levels off after a full
    scroll.
- **Gestures: where he is most right, and where it hurts now.**
  - "The platform recognizes" (LLP 1002 D4) is true only on iOS. The web,
    macOS and Linux each have hand-written recognizers.
  - Each interaction is a one-off attribute implemented on each host
    (`swiperight`, `heightDragFor`, `transformDragFor`, `reorderFor`).
    Recognition is one finger only, with no pinch.
  - The subtree exclusion he wanted exists, but it is hard-coded in the iOS
    host; authors get only `touch-action`.
  - The proposal was one recognizer primitive (pan, pinch, long press)
    reporting began, changed and ended, with velocity, to Contract actions on
    the main thread. iOS keeps UIKit; one shared Rust core replaces the other
    hosts' recognizers; authors declare arbitration. It needs an RFC, and it
    moves DEFERRED's gesture-arena line.
- **Hypothetical layout: exact2 offered none to apps.** The one what-if pass
  (`measure_height_targets`) is native only, host only and returns heights
  only. The follow-up is LLP 1051 (the research, with a ball flung against a
  wall worked through) and LLP 1051.000 (an implementation outline).

## Measured since, 2026-09-27

**Wasm memory at 100,000 records, on the web.** This is part of the data
measurement above. `apps/messages-stress` ran at 100,000 logical messages in
desktop Chrome. Its data is a Rust source inside the wasm, so nothing crosses
to TypeScript here. The probes were scratch scripts, not in the repo. They
read the app's exported memory, captured at instantiation, and the JS heap
over CDP (`Runtime.getHeapUsage`) while scrolling the whole history.

| Mode | Wasm linear memory | JS heap |
|---|---|---|
| Boot | 3.9 MB | 2.8 MB |
| Virtualized transcript: the whole history supplied | 65.1 MB, then 65.7 across repeated full scrolls | 3–7 MB; 3 MB after GC |
| The same, then back to 100 messages | 71.1 MB | 2.8 MB |
| The same, then 100,000 again | 71.1 MB | 3.0 MB |
| Bounded: at most 200 records resident, walked through all 977 windows, from message 99,800 to 0 | 5.1 MB, flat from about the 270th window | peak 12 MB; 4 MB at rest |

- **Memory levels off in both modes.** Freed memory is reused inside the
  wasm: a second 100,000 cost nothing new.
- **Jordan's eviction point holds, in one sense.** Wasm memory never goes
  back. A page that once held 100,000 records keeps about 71 MB until it
  reloads. Evicting inside the wasm helps reuse but returns nothing to the
  browser.
- **Granularity is what matters, as LLP 1027.003 found.** Windowed answers
  held the walk to 5.1 MB against 65–71 MB for the whole history, and they
  are LLP 1027.004's rule.

**The same on an iPhone 17 Pro Max (iOS 26.6.1), natively.** The same app
at 100,000 messages, built for the device at `ed59545d`. It was driven over
the network through the agent, with `wheel` scrolls of 10,000 pt, and ran
until the offset stopped moving each way. Instruments couldn't reach the
phone over Wi-Fi ("timed out waiting for device to boot"). So a local-only
probe, never committed, logged the app's `phys_footprint` (`task_vm_info`)
once a second, and the samples were aligned to the driver's phases.

| Mode | Phone footprint | Web wasm memory, for comparison |
|---|---|---|
| Boot | 18–20 MB | 3.9 MB |
| Virtualized, whole history: load, then rested | 124 MB peak, 113 rested | 65.1 MB |
| The same, across two full passes (0 → 8.3 M pt → 0, twice) | 90–97 MB, level | 65.7 MB, level |
| The same, then back to 100 messages, rested 15 s | **19.8 MB** | 71.1 MB |
| The same, then 100,000 again, rested | 89.7 MB | 71.1 MB |
| Bounded, walked through every window to messages 0–199 (1,011 wheels, 166 s) | 21–24 MB, level; 17 MB at rest | 5.1 MB |

- **Natively, memory goes back.** Dropping from 100,000 messages to 100
  returned the footprint to the boot level within 15 seconds; the session's
  rest trim releases what the allocator freed. On the web the same step kept
  71 MB. Jordan's "hard to know when to evict" is a web problem here, not a
  Rust one: wasm memory can grow and never shrink.
- **Both hosts level off, and loading 100,000 again costs no more than the
  first time.**
- **The whole history costs about 70–75 MB above boot on the phone; the
  bounded window costs 3–5 MB.** Granularity again decides it.

Not measured yet:
- **Crossing cost at this scale.** `messages-stress` has no TypeScript side.
  Messages can't reach 100,000 records today: Hermes' 64 MiB heap and 100 ms
  call budget (`js/src/lib.rs`), Snapback's 512-record edit limit, and a
  transcript that isn't virtualized. It needs a TypeScript-data variant of the
  stress app.
- **Memory tooling for a device.** The repo has none. The heavy-list lanes
  measure `phys_footprint` with a harness outside it, and this measurement
  used a probe it didn't keep.

## To ask Jordan

1. **Preflex.** What it takes and returns, and how it stays in agreement with
   the platform's layout. Nothing public goes by that name (searched
   2026-09-25). His `jordwalke/flex` is a Reason port of Yoga, from 2016.
2. **Subtree exclusion.** His cases for excluding touches above or below a
   subtree while several recognizers run at once.
3. **What his iOS reimplementation missed**, and which behaviours did not port.
