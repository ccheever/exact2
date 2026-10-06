# LLP 1103: Making a fetch fail in a test or a drive

**Type:** RFC
**Status:** Draft r2, 2026-10-06. Charlie accepted the direction (LLP 1102 §0, §3.3). r1 was reviewed blind by Astra (`gpt-6-astra`, xhigh) and Grok 4.7 (xhigh); both found it not ready (§6). r2 narrows the first version to what every executor can do exactly.
**Systems:** the grant check every fetch passes (`grants/` and each executor's fetch path: `js/src/prelude.js` on Hermes, `host/web-js/ts-fetch.js` and `rt.js` `data.fetch` on the JS target, the native HTTP executor for a Rust source's declarative request), the authored-test grammar and runner (`contract/syntax/src/parser/steps.rs`, `scripts/agent-test.mjs`), the agent's open options and operations (`scripts/agent.mjs`, the web, Apple and Linux carriers), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Implementer:** Claude (Opus 5.5) lanes, after review
**Related:** LLP 1102 §3.3 (the finding: 64% of recipe-app trials; the bench's largest time sink) and §3.5 (timeouts); LLP 1016 D4 and D6 (a failure the app catches is journaled; a fetch outside the grant is refused); LLP 1027.002 (module workers); reviews of r1 (§6)

## 1. Summary

An app's error and retry paths cannot be tested today. Builders read the driver's source to find raw CDP (`Network.setBlockedURLs`, web only), wrote fault proxies, or rebuilt against a dead port.

This RFC adds one fault, on every host:

```text
fail fetch "https://api.example.com/recipes"           # every later fetch whose URL starts with this fails
fail fetch "https://api.example.com/recipes" times 1   # only the next one
pass fetch "https://api.example.com/recipes"           # stop failing
```

It is both a test step and a drive operation. In a test it may also be a launch line, so it is armed before the app's first data load.

A matching fetch fails exactly as a real network failure does on that executor. The source's own code runs: its `catch`, its error record, its retry. So the test checks the app's actual error handling.

## 2. Decisions

### D1 — Match by URL prefix, at the grant check

**Why a URL and not a source name.** r1 matched "the next host request made while answering source X". The reviews showed that this attribution does not exist reliably:
- the JS target's `answering.call` is cleared when a source returns its promise, before the `await`s that fetch;
- Hermes interleaves answers;
- a Rust request carries its resource or mutation, not its source;
- a module worker answers off the runner's thread.

**Where.** Every fetch on every executor already passes one choke point that knows its URL: the grant check (LLP 1016 D6). That is where the fault table lives. A fetch whose URL starts with an armed prefix does not go out; it fails at that point.

Matching by URL is also what the builders reached for (CDP's `setBlockedURLs` is URL-shaped), and it is executor-agnostic.

### D2 — The failure is each executor's real network failure

A matched fetch fails through the same path a refused connection takes on that executor:
- **Hermes:** the `fetch` promise rejects with the prelude's `FetchError`;
- **the JS target:** `fetch` rejects as the browser's does (`TypeError: Failed to fetch`);
- **a Rust source's declarative request:** its outcome is `Outcome::Failed { kind: Network }`.

No new error shape is invented. A source that catches the failure and returns a record lands as it would after a real failure. One that throws sets `failed(resource)` as it would.

The journal records each injected failure ("fetch failed (driver fault): https://…/recipes/3"), so a test author can see that the fault fired.

### D3 — Arming

- **A launch line in a test,** before the first step, as `locale` and `epoch` are: `fail fetch "…"`. It is armed before the app boots, so the first data load fails. This covers the most common case, "the API is down when the screen opens".
- **A step later in a test,** affecting every later matching fetch: `fail fetch "…"`, `pass fetch "…"`. With `times N` it fails the next N matching fetches, then passes.
- **A drive:** `--fail-fetch <prefix>` at open (repeatable), and `"fail fetch <prefix>"` and `"pass fetch <prefix>"` as operations on every carrier.

An armed fault lasts until `pass`, its count runs out, or the test or drive ends. A fault with a count that never fired is a test failure ("fail fetch … times 1 matched no fetch"), so a test cannot pass by testing nothing.

`fail` and `pass` are new step words. `hold` stays the drag modifier it already is.

### D4 — Interaction with the runner's request handling

A failed fetch is an ordinary failed request:
- **Deduplication** and **refresh** see a request that ended, as with a real failure. Nothing is left in flight.
- **`clock settle`** and **`clock data`** count it as settled once it has failed.
- **A queued mutation** whose request fails behaves as after a real failure: its reply lands, the queue moves on.

There are no tickets to manage and no held state to retire. That is the reason r2 defers holding (D5).

### D5 — Deferred: holding a request, delays, storage faults

- **`hold`** (a request that stays in flight until released) would make a timeout testable, but the reviews found it interacts with deduplication, `forced` refreshes, `clock settle`'s device-hold rule and session teardown in ways that need their own design. Until then, a timeout is tested against a stand-in that never answers, as the bench's t8-library does.
- **Storage faults** on Hermes start the adapter operation before the runner sees the continuation, so they need a separate interception point. Storage failures are rarer in the bench.
- **A staged status** (`fail fetch "…" with 500`, a response instead of a network failure) is a small extension of D2 if a test needs to exercise HTTP-status handling.

## 3. What it enables

```text
test "the recipes list shows an error and retries"
  fail fetch "https://api.example.com/recipes"
  expect text "error" == "Couldn't load recipes."
  pass fetch "https://api.example.com/recipes"
  tap "retry"
  clock data
  expect text "count" == "12 recipes"
```

The same test runs on the web, iOS, macOS and Linux.

## 4. Cost

| Part | Estimate |
|---|---|
| The fault table beside the grant check on each fetch path (Hermes prelude, the JS target's fetch and `data.fetch`, the native HTTP executor), with the executor's real failure | about a lane-day |
| The launch option through each carrier's launch facts, the drive operations, journal lines | half a lane-day |
| Test grammar (launch line and steps), the test runner, the unfired-fault failure | half a lane-day |
| Tests on each executor; docs (the guide's testing section, the grammar, an error-state recipe) | half a lane-day |

About two and a half lane-days.

## 5. Open questions

1. Prefix or glob? A prefix is enough for the bench's apps. CDP's patterns allow `*`. Proposed: prefix now, `*` later if asked.
2. Should a fault also match a WebSocket or SSE open (`exactStream`)? Proposed: yes for the open, which is a fetch; a stream's later messages are out of scope.

## 6. Revisions

- r1, 2026-10-06: first draft (fail and hold a source's host request, by source name).
- r2, 2026-10-06: Astra and Grok found r1 not ready:
  - source attribution does not survive `await`s, Hermes interleaving, Rust targets or workers;
  - native storage dispatches before the runner sees it;
  - a hold breaks against deduplication, refresh, `clock settle` and teardown;
  - the failure shapes were wrong (`FetchError`, not `TypeError`, on Hermes);
  - a fault could not be armed before the first data load.

  r2 matches by URL prefix at the grant check, uses each executor's real failure, arms on a launch line, and defers `hold` and storage faults.
