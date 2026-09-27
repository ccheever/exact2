# Under the agent, the launch seed, locale and time zone come from the machine and a CSPRNG, and a web dev reload draws a new seed

**Status:** Open
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
