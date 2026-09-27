# Under the agent, the launch seed, locale and time zone come from the machine and a CSPRNG, and a web dev reload draws a new seed

**Status:** Fixed: every agent carrier supplies seed 0, en-US and UTC with drive overrides; web and native reloads retain the launch seed, verified by agent, Linux, Swift and live smoke regressions (broader host-suite failures noted below).
**Systems:** Web host (`host/web/glue.js`), Apple host (`host/apple/Sources/ExactKit/Session.swift`), agent (`scripts/agent.mjs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1027.000.000 D3 and its seed note, LLP 1012 (the clock in the agent's hands)

`host/web/glue.js:1362` (`bootNow`) and `host/apple/Sources/ExactKit/Session.swift:813-823` take locale and zone from the platform and draw the seed with `crypto.getRandomValues` or its equivalent, even under `?agent=1`. `scripts/agent.mjs` never supplies substitutes. LLP 1027.000.000 D3 says the agent supplies these facts.

**Failures:**
- Two agent drives of an app that builds ids from `exactTime().seed`, or that formats with `locale`/`timeZone`, produce different strings and different screenshots.
- A web dev restart draws a fresh seed within one page launch. Apple keeps its session's seed across reloads, so the seed's lifetime depends on the host.

**Fix:**
- Under the agent, read a fixed seed, locale and zone, with documented defaults overridable from the drive (environment or query), and never touch the platform's values.
- Keep the seed for the launch across dev reloads on every host.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Grok 4.7 xhigh, Astra max (code and design). Verification: confirmed by reading.

Implementation and verification (2026-09-27, Astra):

- `scripts/agent-launch.mjs` extracts session flag parsing from the capped
  driver and supplies validated `--seed`, `--locale`, `--time-zone` options
  (also `open({seed, locale, timeZone})`). Defaults, query keys and native
  environment names are documented in `README.md`.
- `host/web/navigation.js` owns the page's seed outside `bootNow`; the
  capped glue only reports it. `Session.swift` chooses explicit agent facts
  without platform locale/zone/entropy reads; Linux uses the same defaults
  and environment overrides. The seed survives runner replacement.
- Reproduced both web failures in `host/web/agent.test.mjs`: the old code
  reported platform facts under the agent and drew twice on reload. Both
  regressions pass; the suite reports `36 pass; 0 fail`, including CLI/env
  overrides, seed bounds and a platform-access refusal double.
- The live smoke fixture checks default and repeated launches, custom facts,
  the `t()` table, and web reload. Linux: `linux smoke: ok in 94.1 s`.
  Web launch-facts assertions and Caltrain's `3 passed, 0 failed` tests pass;
  the overall smoke fails on Chrome keychain/encryption and paint-metrics
  diagnostics.
- The new Swift `testAgentLaunchPlaceDefaultsAndOverrides` passes. Both full
  macOS XCTest runs report `Executed 443 tests, with 1 failure`, in the
  unchanged `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
  at line 157 (`seen.count` 1, expected 240); it is recorded in `QUEUE.md`.
- Root build, test (`1715 passed; 0 failed; 8 ignored`), Clippy and fmt pass;
  host Clippy passes. Caps and boot checked with the final staged change.
  No design ruling remains. Wall-date reporting and elapsed clock behavior
  are separate from these three launch facts.
- Final web rerun: `web smoke: 1 failure(s) in 43.1 s`; all functional
  assertions, including launch facts, pass. Only Chrome's macOS keychain
  (`errSecInteractionNotAllowed`) and unavailable password encryption remain.
  Stopped after the third web attempt (first: missing offline dependency;
  fetched the locked dependency, then two completed functional sweeps).
