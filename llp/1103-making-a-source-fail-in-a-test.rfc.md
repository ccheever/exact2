# LLP 1103: Making a source fail (or wait) in a test or a drive

**Type:** RFC
**Status:** Draft r1, 2026-10-06. Accepted in principle by Charlie (LLP 1102 §0, §3.3); this document is the design he asked for before building.
**Systems:** the runner's request path (`runner/src/request.rs`, `runner/src/runner/commit.rs`), the native data executor (`js/src/prelude.js`, the Hermes host request path), the JS target's data layer (`host/web-js/ts-data.js`, `ts-fetch.js`, `rt.js`), the agent's carriers (`host/web-js/agent.js`, the Apple and Linux agents, `scripts/agent.mjs`), the authored-test grammar and runner (`contract/syntax/src/parser/steps.rs`, `scripts/agent-test.mjs`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Implementer:** Claude (Opus 5.5) lanes, after review
**Related:** LLP 1102 §3.3 (the finding: 64% of recipe-app trials, the bench's largest time sink) and §3.5 (timeouts); LLP 1016 D4 (a failure the app catches is still journaled); LLP 1027.001 (storage outcomes); LLP 1012 (the agent's operations); LLP 1092 (gated tasks: a deadline needs a request that does not answer)

## 1. Summary

An app's error and retry paths cannot be tested today. A test that wants "the recipes API is down" has no step for it. Builders read the driver's source to find raw CDP (`Network.setBlockedURLs`, web only), wrote fault proxies, or rebuilt against a dead port.

This RFC adds two faults, as a drive operation and a test step:

```text
fail "recipes"               # the next host request made while answering `recipes` fails
fail "recipes" times 3       # the next three
hold "recipes"               # the next such request stays in flight until released
release "recipes"            # release every held request of `recipes` (answers it normally)
```

A fault acts on a **host request** (a `fetch`, a storage operation) made while a source answers, not on the resource's `failed()` flag. The source's own code runs and sees the failure exactly as it would see a real network error: its `catch` runs, it may return an error record, or it may throw. So the test checks the app's actual error handling (LLP 1102 §3.3, the reviewers' finding).

## 2. Decisions

### D1 — A fault is a failed host request, attributed to the source being answered

**The fault.** `fail "name"` arms a fault for the next host request issued while the source `name` is answering. That request does not go out. Its outcome is `Outcome::Failed { kind: Network, message: "injected by the driver" }`:
- in a TypeScript source, the `fetch` promise rejects with a `TypeError` (the web's network-error shape) and a storage call rejects with its coded error;
- in a Rust source, its request's reply is that outcome.

**Attribution.** A request is attributed to the source whose answer issued it:
- on the JS target, `ts-fetch.js`'s `answering.call` already names it;
- on native, the Hermes executor runs one answer at a time per instance and knows the source it was asked for;
- a Rust source's requests carry its ticket.

A request issued outside any answer is never matched: a mutation's `then` action, a stream already open.

**Mutations.** `name` may name a mutation's source too (`fail "saveRecipe"`), for testing a failed save. The first version covers resources and mutations that answer once. Streams are out of scope (D5).

### D2 — `hold` keeps a request in flight, and the driver does not wait for it

`hold "name"` arms a hold: the next attributed request is issued to no one, and stays in flight.

The drive must not wait on it:
- `clock settle` and `clock data` count held requests out, as they count the auth and file-picker device holds (`holds()` in `host/web-js/agent.js`);
- a held request otherwise behaves as a real slow request (`pending(r)` stays true; a gated task's deadline fires under `clock +N`).

**Release.**
- `release "name"` sends every held request of `name` out for real, in the order it was held, and its real answer lands.
- A held request still open when the drive or test ends is aborted (`Outcome::Failed { kind: Aborted }`) and journaled.

r1 of LLP 1102 proposed release by ticket. Release by source name is simpler for a test to write, and a source rarely has more than one request in flight at once. A ticket form can be added if a test needs to release one of several.

### D3 — The test steps and the drive operation share one grammar

- **In a test file:** `fail "name"`, `fail "name" times N`, `hold "name"` and `release "name"` are steps. A fault armed in a test ends with the test: an unconsumed fault is reported as a failure of the test ("fail \"recipes\" was armed and no request was made").
- **In a drive:** the same words are an operation on every carrier (`agent web …`, `agent ios …`). The reply names what was armed, and the journal records each consumed fault.

### D4 — Every host, in the runner or beside it

The fault table lives where the host request is decided:
- **Native (Apple, Linux, Windows):** in the runner's request path, before the executor (`request.rs`).
- **The JS target:** in its data layer, before `fetch` and the storage adapters (`ts-data.js`).

The agent operation on each carrier writes the table. CDP is not used: it is web-only, and it fails a URL, not a source.

### D5 — What is out of scope for the first version

- **Streams** (`exactStream`, Rust streamed requests): a failed stream open is a later decision, because a stream's messages and its end are separate outcomes.
- **Delays by a given time** (`delay "name" 2000`): `hold` plus `clock +N` covers a timeout test.
- **A failed answer without a request:** a synchronous source that issues no request is never matched; a test makes it fail by the data it reads.

## 3. What it enables

- An authored test of a recipe app's error state and retry, on the web and iOS, in four lines.
- LLP 1102 §3.5's timeout recipe, demonstrated: `hold "recipes"`, `clock +10000`, expect the deadline's state, `release "recipes"`, expect the late answer's policy.
- A failed save (`fail "saveRecipe"`) and the app's message for it.

## 4. Cost

| Part | Estimate |
|---|---|
| Native fault table and attribution in the runner's request path; abort at session end | about a lane-day |
| The JS target's data layer (fetch and storage), and `holds()` counting | half a lane-day |
| The agent operation on the web, Apple and Linux carriers; replies and journal lines | half a lane-day |
| Test grammar, the test runner, unconsumed-fault reporting | half a lane-day |
| Docs (the guide's testing section, the grammar, a recipe for an error state and a timeout) and fixtures | a few hours |

About three lane-days, within LLP 1102's estimate for resources and once-answering mutations.

## 5. Open questions

1. Is `Network` the right injected kind for storage too? Storage errors are coded (`ENOENT`, …). The alternative is a storage-shaped error (`EIO`) for storage requests.
2. Should `release` accept an optional answer (`release "recipes" with 500`) to stage a server error rather than the real answer? That would test HTTP-status handling without a server. It would make D2 a small fixture system. Defer unless a test needs it.

## 6. Revisions

- r1, 2026-10-06: first draft.
