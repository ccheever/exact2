# Boot module count does not detect growth inside an allowed module

**Status:** Open
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
