# Independent output reads and scroll boundary repair

The first mounted replay showed that the shared read mutation still delayed a second row until the first request finished. Disclosure now publishes through a dedicated immediate mutation. Its completion dispatches two independent root read slots. Each source invocation claims one eligible row before awaiting its own request; further rows remain queued for the root tick. This is bounded two-request concurrency, not unbounded parallel reads.

The same replay showed that repeated Down presses at the bottom accumulated an authored scroll position beyond the native scroll limit. Down and Space now clamp to the measured content height minus viewport height, allowing the next Up press to move immediately.

`checks-two-read-slots/report.json` records 1,200 passing Bun tests, strict TypeScript, Contract compilation, Rust formatting, root build/clippy and boot checks. Source was unchanged throughout the run. The native bundle build passed in `two-read-slots-native-build.log`.

The current manifest names `T3 Code (Exact).app`. The rebuilt named bundle and executable are recorded in `named-bundle-check.json`; the standard receipt freshness check returned no changed inputs before launch. Final runtime replay uses that bundle executable with `EXACT_MAC_BIN`, an isolated agent data directory and the existing pinned real-server fixture. Final behavior verdicts are recorded in the timeline and activity attempt directories; these static checks alone do not close either task.
