# Measure a 1,000-row list whose rows declare `transition: all`

**Status:** Closed
**Resolution:** Fixed: Native paint adopts each transition target on its first change, eliminating 9,000 idle slots for 1,000 `all` rows; Chrome/macOS measured before and after, first-transition and lifecycle regressions plus all required checks verified. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Heavy-list harness, `exact-motion`, Apple/web hosts
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062 D2/D3; the memory work in QUEUE

LLP 1062 D2 says only a node that names paint owns it, so 1,000 coloured rows cost nothing. But `all` names every property, so rows that declare `transition: all`, as web authors routinely do, each become paint owners, with engine slots per property (about 104 B each) and whole-style re-sends on Apple.

**Do:** measure memory and frame time for 1,000 rows with `transition: all` on the heavy-list harness, on iOS and the web. If the cost is real, adopt ownership lazily, at the first change of a paint target, rather than at declaration.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: Native paint adopts each transition target on its first change, eliminating 9,000 idle slots for 1,000 `all` rows; Chrome/macOS measured before and after, first-transition and lifecycle regressions plus all required checks verified.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

## Measurement and change

Measured 2026-09-27 by Astra on an Apple M5 Pro, macOS 27.0 (26A428),
Chrome 154.0.8037.58. Baseline: `9068ef6d`; after: this change. The referenced
`~/bench/heavybench` and `~/bench/cryptobench` were absent. This uses the available
`~/bench/exact-listbench/app.contract`'s `MessageRow`, expanded into 1,000 mounted
rows under a scroll view, with its four rotating messages, borders and attachment
badges (12,308 live nodes). No virtualization or images; this is a declaration-cost
isolation, not a reproduction of QUEUE's device heavy-list/crypto footprint runs.
Only the row container differs: no transition versus `transition="all 1s linear"`.
A common 40×40 indicator transitions white to black for one second; then all row
backgrounds change from white to `#1d4ed8`. The plain rows snap in that second case.

Private plans and raw results are in this worktree's ignored `target/transition-all/`;
no benchmark script, app or check is added. Build the existing `svg-gallery-web`
and `svg-gallery-apple` with `host/web/build.mjs` and `host/apple/build.mjs`; use
`scripts/agent.mjs` (`open`, `tap`, `clock`, `carrier.evaluate`) with `--app svg-gallery`
and each private `--plan`, at 420×900. Rebuild both hosts for the after pass.
Three fresh-process runs per variant, alternating none/all, all/none, none/all.
Before each run and each timed host phase, read `sysctl -n vm.loadavg`; if its
one-minute value exceeds 4, poll every 30 seconds until it does not. No admitted
sample exceeds 4. Read RSS with `ps -axo pid=,ppid=,rss=,command=` immediately after
`clock 0` and after both changes have settled. macOS is the app PID; Chrome is the
sum of renderer descendants of the recorded, isolated browser PID (including its
spare renderers). RSS is not malloc-in-use or a physical-device footprint.

Chrome: restart the observed CSS animations at time zero and play them, then
collect 60 consecutive `requestAnimationFrame` intervals. There is one indicator
animation in both variants and 0/1,000 row animations without/with `all`.
Native: time 60 awaited `clock` requests at 1/60-second increments during each
change, including Rust, Swift application and agent IPC. These are **virtual-frame
round trips, not display-link intervals or physical FPS**; do not compare their
absolute values with Chrome's vsync intervals. Separately, an optimized scratch
Rust probe links this worktree's `apple-dev` host, boots the same plans with
`MonospaceMeasurer`, counts nonempty `Engine::target`s across every live node and
property, and times 59 `Host::tick` calls at the same increments. That probe
isolates Rust frame production, excluding text rasterization, Swift and IPC.
Tables summarize the median of each run's p50/p95, not a pooled percentile.

The native cost is real in memory: baseline `all` adds 9,000 engine paint
slots before any row changes. A settled slot plus its key is now 56 bytes (the
ticket's 104-byte estimate predates the compact-slot change), before hash-table
capacity and ownership bookkeeping. Lazy adoption removes those slots; it still
keeps the exact before-change targets in a smaller per-node vector. Keyframes and
exits retain their underlying values immediately. First appearance reports update
pending targets without motion; retirement and destruction discard them.

The median paired macOS declaration RSS surcharge is 3.281 MiB before and
1.422 MiB after (individual differences: 3.281, 4.516, 0.938 before;
2.406, 1.422, -0.797 after). Absolute `all` RSS falls 333.219 → 329.359 MiB,
while the plain control is 329.266 → 329.906 MiB. This supports a memory saving,
not an exact allocation-size claim: the per-run RSS spread is visible below.
There is no demonstrated frame-time win from removing idle ownership. Rust's
1,000-row moving-paint p50 rises 1.570 → 1.610 ms (+0.040 ms, 2.5%);
three runs do not separate that small difference from run noise. No CPU speedup
is claimed. Active rows still emit the same presented styles.
Chrome remains at 16.7 ms p50 in every run; its native paint engine slot count is
zero because CSS executes the transitions.

| Pass | Host | Rows | RSS at rest / after changes (MiB) | Indicator p50 / p95 (ms) | All-row change p50 / p95 (ms) |
|---|---|---|---:|---:|---:|
| Before | macos | none | 329.266 / 369.125 | 19.549 / 21.481 | 7.032 / 8.162 |
| Before | macos | all | 333.219 / 374.672 | 19.851 / 22.988 | 43.909 / 49.459 |
| Before | web | none | 813.688 / 848.203 | 16.700 / 16.800 | 16.700 / 16.700 |
| Before | web | all | 812.266 / 861.328 | 16.700 / 16.800 | 16.700 / 16.800 |
| After | macos | none | 329.906 / 368.422 | 18.144 / 19.397 | 6.306 / 6.675 |
| After | macos | all | 329.359 / 372.469 | 18.653 / 21.759 | 39.594 / 43.340 |
| After | web | none | 813.469 / 847.938 | 16.700 / 16.700 | 16.700 / 16.800 |
| After | web | all | 812.750 / 861.906 | 16.700 / 16.800 | 16.700 / 16.700 |

| Pass | Rows | Paint / total engine slots at boot | Indicator p50 / p95 (µs) | All-row change p50 / p95 (µs) |
|---|---|---:|---:|---:|
| Before | none | 1 / 49233 | 1.000 / 1.208 | 0.167 / 0.209 |
| Before | all | 9001 / 58233 | 1.875 / 2.000 | 1570.042 / 1673.167 |
| After | none | 0 / 49232 | 1.000 / 1.250 | 0.167 / 0.250 |
| After | all | 0 / 49232 | 1.750 / 2.042 | 1610.042 / 1744.166 |

The Rust probe reports the same slots on all three runs of each case. Mean
batch bytes per sampled frame are unchanged: indicator 191 bytes, plain rows 78,
`all` rows 301,755. Native-probe loads are 3.90 before and 3.65 after. Host loads
are 3.27–4.00 before and 3.29–3.70 after; every individual admitted load and run
is recorded in the ticket. The load gate also held during builds and busy periods.

## Individual runs

| Pass | Host | Rows | Run | Load: launch / indicator / rows (1 min) | RSS rest / after (KiB) | Indicator p50 / p95 (ms) | Rows p50 / p95 (ms) |
|---|---|---|---:|---|---:|---:|---:|
| Before | macos | none | 1 | 3.90 / 3.90 / 3.74 | 337856 / 377648 | 19.549 / 21.227 | 6.717 / 7.098 |
| Before | macos | none | 2 | 3.84 / 3.84 / 3.84 | 336768 / 378384 | 19.299 / 21.481 | 7.032 / 8.162 |
| Before | macos | none | 3 | 3.84 / 3.84 / 3.70 | 337168 / 377984 | 20.937 / 26.528 | 7.239 / 11.329 |
| Before | macos | all | 1 | 3.74 / 3.74 / 3.74 | 341216 / 383664 | 19.660 / 22.988 | 44.499 / 49.459 |
| Before | macos | all | 2 | 4.00 / 4.00 / 4.00 | 341392 / 385008 | 19.851 / 20.854 | 43.909 / 46.284 |
| Before | macos | all | 3 | 3.70 / 3.70 / 3.70 | 338128 / 381424 | 22.113 / 25.742 | 43.349 / 49.531 |
| Before | web | none | 1 | 3.59 / 3.59 / 3.59 | 833472 / 869296 | 16.700 / 16.800 | 16.700 / 16.700 |
| Before | web | none | 2 | 3.27 / 3.27 / 3.27 | 832384 / 868000 | 16.700 / 16.800 | 16.700 / 16.800 |
| Before | web | none | 3 | 3.27 / 3.27 / 3.49 | 833216 / 868560 | 16.700 / 16.700 | 16.700 / 16.700 |
| Before | web | all | 1 | 3.59 / 3.38 / 3.38 | 831424 / 881440 | 16.700 / 16.800 | 16.700 / 16.800 |
| Before | web | all | 2 | 3.38 / 3.38 / 3.38 | 831760 / 882000 | 16.700 / 16.800 | 16.700 / 16.800 |
| Before | web | all | 3 | 3.49 / 3.49 / 3.49 | 832864 / 882576 | 16.700 / 16.700 | 16.700 / 16.700 |
| After | macos | none | 1 | 3.44 / 3.44 / 3.44 | 330752 / 370848 | 18.131 / 19.711 | 6.266 / 6.518 |
| After | macos | none | 2 | 3.29 / 3.29 / 3.29 | 337824 / 377264 | 18.144 / 19.215 | 6.306 / 6.675 |
| After | macos | none | 3 | 3.29 / 3.67 / 3.67 | 338080 / 378832 | 18.414 / 19.397 | 6.402 / 9.426 |
| After | macos | all | 1 | 3.44 / 3.44 / 3.40 | 333216 / 375968 | 18.653 / 22.521 | 39.594 / 43.340 |
| After | macos | all | 2 | 3.40 / 3.40 / 3.40 | 339280 / 383344 | 17.823 / 19.091 | 38.800 / 41.127 |
| After | macos | all | 3 | 3.67 / 3.67 / 3.67 | 337264 / 381408 | 18.912 / 21.759 | 41.338 / 45.710 |
| After | web | none | 1 | 3.53 / 3.53 / 3.53 | 832992 / 868288 | 16.700 / 16.700 | 16.700 / 16.700 |
| After | web | none | 2 | 3.70 / 3.70 / 3.70 | 833200 / 869168 | 16.700 / 16.800 | 16.700 / 16.800 |
| After | web | none | 3 | 3.70 / 3.70 / 3.64 | 832944 / 868176 | 16.700 / 16.700 | 16.700 / 16.800 |
| After | web | all | 1 | 3.41 / 3.41 / 3.41 | 832384 / 882592 | 16.700 / 16.800 | 16.700 / 16.700 |
| After | web | all | 2 | 3.41 / 3.41 / 3.70 | 832000 / 881360 | 16.700 / 16.800 | 16.700 / 16.700 |
| After | web | all | 3 | 3.64 / 3.64 / 3.64 | 832256 / 882896 | 16.700 / 16.700 | 16.700 / 16.800 |

| Pass | Rows | Run | Load (1 min) | Indicator p50 / p95 (µs) | Rows p50 / p95 (µs) |
|---|---|---:|---:|---:|---:|
| Before | none | 1 | 3.90 | 1.000 / 1.083 | 0.167 / 0.208 |
| Before | none | 2 | 3.90 | 1.000 / 1.208 | 0.167 / 0.209 |
| Before | none | 3 | 3.90 | 1.042 / 1.250 | 0.167 / 0.209 |
| Before | all | 1 | 3.90 | 1.875 / 2.000 | 1570.042 / 1745.584 |
| Before | all | 2 | 3.90 | 1.834 / 1.959 | 1572.375 / 1673.167 |
| Before | all | 3 | 3.90 | 1.875 / 2.000 | 1554.958 / 1649.250 |
| After | none | 1 | 3.65 | 1.000 / 1.334 | 0.166 / 0.250 |
| After | none | 2 | 3.65 | 1.000 / 1.166 | 0.167 / 0.250 |
| After | none | 3 | 3.65 | 1.000 / 1.250 | 0.167 / 0.209 |
| After | all | 1 | 3.65 | 1.750 / 2.042 | 1568.375 / 1633.625 |
| After | all | 2 | 3.65 | 1.875 / 2.042 | 1636.875 / 1824.125 |
| After | all | 3 | 3.65 | 1.750 / 1.875 | 1610.042 / 1744.166 |

## Verification

The new `a_thousand_transition_all_rows_adopt_only_the_paint_that_changes` failed
on the baseline (`Color` already had a slot), then passed. It checks idle slots,
an unchanged target, the first transition's exact start and midpoint, reversal,
and settlement. `retiring_a_pending_target_forgets_its_before_change_value` checks
removal/redeclaration. Existing appearance and reused-generation tests now check
pending ownership and the next transition's starting value.

Required checks, with the final source:

- `cargo build --all-targets --keep-going`: `Finished dev profile ... in 10.05s`.
- `cargo test --lib --bins --tests --no-fail-fast`: aggregate `1771 passed; 0 failed; 8 ignored` across 71 test binaries.
- `cargo clippy --all-targets --keep-going -- -D warnings`: `Finished dev profile ... in 2.86s`.
- `cargo fmt --all -- --check`: exit 0.
- `git add -A && bun scripts/caps.mjs`: `All budgets within cap. 6 categories inspected.`
- `bun scripts/boot.mjs`: `modules reachable before first pixel: 2`; `Allowed import paths only.`

Host checks: Apple paint tests `13 passed; 0 failed`; Linux pinned paint tests
`5 passed; 0 failed`. Both web and macOS apps rebuilt and were driven for every
measurement. Native tests cover first-transition premultiplication, shadows,
`currentcolor`, inherited text, appearance changes, SVG paint and exit keyframes.

No design ruling remains for Charlie. The unavailable original heavy-list harness
and lack of physical display-link timing are explicit measurement limits above;
these numbers make no iOS-device, virtualized-list or physical-FPS claim.
