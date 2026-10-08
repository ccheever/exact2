Three **should-fix** correctness issues:

1. **Short padded lists request unreachable offsets.** [into_view.rs:167](/private/tmp/bsky4-rv/wt-inset/runner/src/instance/collection/into_view.rs:167), [index.rs:632](/private/tmp/bsky4-rv/wt-inset/runner/src/instance/collection/index.rs:632), [list.js:619](/private/tmp/bsky4-rv/wt-inset/host/web-js/list.js:619).

   The upper bound still floors the row-relative offset at zero, although the physical end can now be negative. With a 600-point port, four 100-point rows, padding 92/83 and matching scroll-padding, requesting the last row with `start` or `center` requests physical offset 92. The entire content fits, so the host clamps to zero; the request eventually ends `unconverged`. I reproduced this in the JS engine. The Rust implementation has the same defect, affecting Apple, Linux and web wasm.

   Use `max(-leading, rows + trailing - port)` for the physical upper bound, separately from nonnegative row lookup coordinates. Apply it to request settlement and end detection too: short lists also leave `scroll-start="end"` armed. Add four- and five-row cases on both axes. This contradicts [LLP 1010’s true-range clamp](/private/tmp/bsky4-rv/wt-inset/llp/1010-scrolling-v1.spec.md:1124); CSS scroll-padding changes the targeting region without changing layout. [CSS Scroll Snap](https://drafts.csswg.org/css-scroll-snap-1/#scroll-padding)

2. **Padding-only changes can produce no feedback, leaving end-follow stale.** [runner/collection.rs:113](/private/tmp/bsky4-rv/wt-inset/runner/src/runner/collection.rs:113), [collection-glue.js:444](/private/tmp/bsky4-rv/wt-inset/host/web/collection-glue.js:444).

   Consider an idle, fixed `box-sizing="border-box"` list following its end. A timer increases only `padding-bottom` from 83 to 109. The port, rows and offset remain unchanged. Both web targets suppress the unchanged collection snapshot; the resize observer compares unchanged border-box dimensions and schedules nothing. I reproduced zero new reports, leaving the port 26 points short. [Apple’s comparison](/private/tmp/bsky4-rv/wt-inset/host/apple/Sources/ExactKit/Collection.swift:669) and [Linux’s comparison](/private/tmp/bsky4-rv/wt-inset/host/linux/src/presenter/collection.rs:942) likewise omit these insets.

   Invalidate collection feedback when resolved insets change, including changes that preserve dimensions. Include insets in host deduplication. The new padding-growth test misses this because its [simulated host](/private/tmp/bsky4-rv/wt-inset/contract/cli/tests/it/collection_inset.rs:89) unconditionally supplies feedback; add a test through the actual host scheduling path.

3. **Growing leading padding loses a followed end even when feedback arrives.** [inset.rs:31](/private/tmp/bsky4-rv/wt-inset/runner/src/instance/collection/inset.rs:31), [inset.rs:48](/private/tmp/bsky4-rv/wt-inset/runner/src/instance/collection/inset.rs:48), [list.js:566](/private/tmp/bsky4-rv/wt-inset/host/web-js/list.js:566).

   With 100×100-point rows, a 600-point port and padding 92/83, the physical end is 9575. Increase top padding to 118 while stationary. The reported row-relative offset changes from 9483 to 9457. `report_anchor` treats that origin shift as being away from the end: my JS probe changed `follows` from true to false and produced no correction, although the new physical end is 9601.

   Preserve the previous leading inset and translate the incoming offset into the previous coordinate system before deciding whether the end was followed. Handle leading changes alongside trailing changes, including the fast path. Add leading-padding growth tests on both axes.

The added tests exercise useful cases, but the web conformance fixture contains no scroll-padding requests, and the tall-row test omits the documented “partly above, align end” case. The LLP also records that the iOS refresh-placement test has not run. I found no additional serious refusal issue.

I read all six commit diffs and ran in-memory JS probes with mocked geometry. I did not run builds or native/browser applications; the checkout remains unchanged.

**These correctness issues block landing.**