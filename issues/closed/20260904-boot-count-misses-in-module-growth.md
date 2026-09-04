# Boot module count does not detect growth inside an allowed module

**Status:** Closed
**Resolution:** Boot reports reachable bytes and hashes, while metrics records built artifacts, observed startup work, distinct paint/DOM/action measurements and their limits without adding a gate.
**Systems:** Boot, Web host, Metrics
**Severity:** P2
**Author:** Codex, at Charlie Cheever's request
**Date:** 2026-09-04
**Related:** rules/RULES.md §Time budgets/§Scope; LLP 1007 §8; scripts/boot.mjs; scripts/metrics.mjs

`node scripts/boot.mjs` passed on 2026-09-04 with one module,
`host/web/glue.js`, then 854 lines. It parses imports and restricts allowed
paths, but does not measure work or byte growth inside that allowed module.
The module includes the Castle-specific listener tracked separately in
`issues/20260904-castle-policy-in-shared-hosts.md`.

The count remains a useful import-boundary check. It does not prove that
all allowed-file code is generic host code or that first-pixel cost stayed
constant. No new startup regression is claimed from the line count.

Use existing boot/metrics tooling to make reachable byte growth and actual
startup work visible alongside module count. Compare a controlled increase
inside the same module with an added import, attribute costs to a fixed
artifact, and retain measurements of useful content and first interaction
as well as first paint. Include the already shipped wasm in the size/cost
accounting; classify the on-demand GPU and data executors by when they load.

Done when the report makes same-module growth visible and its wording no
longer treats an allowed path as proof of no app-specific code. Keep the
existing deterministic import restriction and the five-check budget. Any
new numerical blocking budget needs an explicit trade and measured baseline;
this issue authorizes neither a sixth check nor a flaky timing gate.

## Investigation and fix (2026-09-04)

Implemented by Codex at Charlie's request, in the existing `boot` and
`metrics` tools. `boot --json` reports each reachable source module's byte
length and SHA-256, the HTML bytes and violations. Its success wording now
states only that import paths are allowed. There is no new numerical budget
or check. Controlled copied-source experiment:

| Input | Modules | Reachable JS bytes | Result |
|---|---:|---:|---|
| Unchanged glue | 1 | 38,387 | pass |
| Same glue plus a 64 KiB comment | 1 | 103,929 | pass, growth visible |
| Glue plus one local import | 2 | 38,434 | refused, extra module named |

`metrics --json` inventories and hashes the actual built web artifacts,
including wasm and on-demand artifacts. It records every served file's hash
and browser Resource Timing entry, with the observed load phase. Diagnostic
CDP instrumentation (never inserted into shipped HTML/glue) records wasm
instantiation spans, long tasks, layout/style/script totals, DOM text
presence, Paint Timing entries and a real CDP click's input-to-DOM latency.
`--interaction <testId>` chooses another app's action; Caltrain defaults to
its station chooser. Nonempty text is a useful-content proxy, not a promise
that arbitrary app data has loaded. A rendering opportunity is not labeled
as a confirmed presentation.

The first observed Caltrain build was `d8b3d057c463f789a6f7092ec4667f3fc459d65ad0b6298bdc3469c0d9a157d8`
(the digest of its public file inventory). Its app wasm was 554,527 bytes,
glue 38,387 bytes; the GPU then fetched 5,807 + 65,383 bytes of JS and
199,666 bytes of wasm. On Chrome 152 / this M5 Max, script-to-DOM was
20.0 ms, navigation-to-nonempty-text 45.3 ms, and the first station-chooser
click-to-changed-DOM 5.6 ms. This was one instrumented, empty-profile,
localhost run. It observed a 164 ms long task while loading optional GPU
work after the DOM. No Paint Timing entry was supplied by this headless
run: paint and contentful-paint were reported unmeasured, not replaced
with the earlier pre-paint rAF callback timestamp.

That observation also exposed the single-rAF GPU scheduler: it ran before
that frame's rendering opportunity. The Castle cleanup lane moved the
initial and reload GPU scheduling to two rAF callbacks and renamed the
host stamp `frameCallbackMs`. This permits a rendering opportunity first;
it does not force unsupported browsers to implement Paint Timing. The
integrated build's metrics must be rerun for its new artifact identity.
Web data in this build is Rust in app.wasm; TypeScript's web executor is
not yet implemented. The report retains each runtime-loaded JS artifact
instead of assuming source-file import restrictions describe runtime work.

The existing caps/boot suite passed all 40 cases. Byte growth remains
visible but nonblocking: choosing a numerical byte or timing budget still
requires the existing explicit budget trade.
