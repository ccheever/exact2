I'll review only the tip commit, starting with the project rules and the findings that commit claims to fix.The review stays read-only and covers only HEAD. Next I’ll read the findings note and the commit itself.The findings note names three host bugs. I’ll read the commit’s runner and host diffs against those claims.The arithmetic and the host report paths are the risky part. I’ll read those functions in place, including clamps and the first-report path.The remaining check is whether Apple's plain-scroller anchoring runs on virtualized lists. I'll confirm `collections.owns` before writing the review.`owns` is true once a list has a snapshot, and plain-scroller anchoring is skipped for those ids. I'll confirm the padding-report and anchor-translation paths still match on each host.Nothing blocks landing. The three Astra findings and the negative `from` parse are fixed on the runner and on every host, and the new tests fail if those fixes are removed.

## Short-list end

The physical end is `max(-leading, rows + trailing - port)` in `runner/src/instance/collection/index.rs:654` and `host/web-js/list.js:123`. Follows-end is judged on that unfloored offset (`index.rs:579`, `list.js:144`), so a short list sitting in its leading padding still counts as the end. `scrollIntoView` clamps into that range (`into_view.rs:167`, `list.js:625`). `at_target` compares the raw negative offset when the target is negative (`start.rs:108`, `list.js:20`); the restore and fast-path call sites pass that raw offset (`mod.rs:732`, `within.rs:84`). `nest.rs:207` uses `min`/`max` so a negative end does not hit `f64::clamp`'s panic. Both axes go through `Insets::main`.

`a_short_list_ends_in_the_padding_before_its_rows` (the index test and the integration test) fails without this. The integration test's `into()` requires `"status":"done"`, and `settle_target` compares the aligned offset to the raw report: a target floored at 0 against a host report of −92 or −17 never settles. `scroll_start_end_on_a_short_list_opens_and_lets_go` fails too: `settle_start` (`start.rs:87`) stays armed, and `more(3)` follows.

## Padding-only feedback

A fixed border-box list changes neither size nor offset, so the old dedup sent nothing.

- Apple: `padding` is on `CollectionFacts` (`Collection.swift:25`) and `flush` uses full equality (`Collection.swift:674`). `endBatch` dirties every live list (`Collection.swift:434`), and `Session.swift:1035` flushes after `apply`, once style ops have landed. Geometry reads main-axis padding from the resolved style numbers (`CollectionIOS.swift:63`, `CollectionMac.swift:272`). An `env(safe-area-inset-*)` change is re-sent as points (`host/apple/tests/it/host.rs` already locks `padding_top: 62`), so `node.number` sees it. The new Equatable test fails if `padding` drops out of equality.
- Linux: the dedup key is `(feedback, padding)` (`presenter/collection.rs:947`). `a_padding_only_change_reaches_the_runner_and_the_end_follows_it` fails if padding is omitted: the grow sends no report and the scroll stays at the old end.
- Web, both targets: `collection-glue.js:577` `restyled()` enqueues when computed main-axis padding differs from `lastFacts`. The report signature already includes leading and trailing (`collection-glue.js:283`), so the follow-up is not dropped, and `lastFacts` is updated so it does not loop. The wasm host calls it when a commit has no collection snapshot (`glue.js:862`); the JS host calls it from `publish` when the snapshot text is unchanged (`list.js:763`), from `After.push` (`list.js:783`) after the DOM commit. The new glue test fails if `restyled` does not enqueue. It calls `controller.restyled()` directly; the two call sites are the wiring above.

Scroll-padding stays out of the dedup on purpose. Only `scrollIntoView` reads it, fresh on each report.

## Leading-padding translation

`report_anchor` (`inset.rs:65`, `list.js:571`) captures on the old range at `offset + (newLeading - oldLeading)` when a geometry already exists, then applies the padding. If that misses a followed end, it recaptures on the new range at the raw offset. A followed end therefore stays followed, and a mid-list reader keeps the same rows. Before the first report `moved_down` is 0. While `at_end` is set, the capture is at `scroll_extent` and a follows-end restore returns the new `max_offset`, so opening still lands on the padded end.

The host must not also shift the port. Web sets `overflow-anchor: none` on the list (`collection-glue.js:491`). Apple's plain-scroller anchoring is skipped for any id in `entries` (`Collection.swift:460`, `PresenterIOS.swift:740` and `:960`, `PresenterMac.swift:904` and `:1120`). A live virtualized list is entered in `beginBatch` before that capture. `a_followed_end_follows_the_padding_before_the_rows_as_it_grows` fails without the translation: the simulated host stays at the old physical position instead of `end + 26`, on both axes, including the reader at physical 5000.

## Negative anchor `from`

`Collection.swift:183` `signed` keeps a finite negative `from`. Absent `from` stays nil, so `takeShift` still requires one. `testMalformedSnapshotsDoNotBecomeGeometry` asserts `from == -92` and fails if parsing goes back to nonnegative `number`.

## Regressions checked

Padding 0 leaves `max_offset` at `(total - port).max(0)`. `at_target` still folds the report with `offset.max(0)` whenever the target is ≥ 0, and `nest.rs`'s `min`/`max` matches the old `clamp(0, max)` for a finite nonnegative end. Horizontal lists use left/right in the runner, both Apple geometries, Linux geometry, and the JS axis table; the new integration tests run both axes. Non-finite padding is forced to 0 before the clamp, so `max_offset >= -leading` and `clamp` does not see `min > max`. A NaN offset is rejected by Rust `validate` and by `collectionBytes` before either engine captures. `restyled()` is one `getComputedStyle` per mounted list on a commit that did not change a snapshot. It enqueues only when padding actually changed.

No findings. Nothing blocks landing.
