---
name: 20261005-x14-parked-native-reply
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-clone-on-exact2-main, 20261005-managed-codex-chatgpt]
upstream_url: null
reproduced_on: null
---

# X14: A let-go answer's in-flight native request survives and hands its reply to the next answer

## Summary

T3 Code's client never loses a server update: a reply that is in flight when the UI asks again is still applied. In exact2, a data resource that awaits `native.later(...)` can lose its reply: when the topic it watches changes, Exact asks the answer again, forgets the first answer, and drops the first answer's pending native reply, so the resource keeps its old value. The clone works around this with a native "read gate" and request tags. An earlier session also carried a framework change for it. What is needed is that a re-ask with equal arguments keeps the in-flight native request and gives its reply to the new answer.

## Why this issue arose

### The T3 Code behavior

The reference client applies every result and every stream frame to its store, in order, and has no notion of an answer being let go. Reads that are still pending when new events arrive finish and are applied. The reference states this expectation in a test of its RPC session: "keeps reading replies after closing a stream with a full buffer" (`packages/client-runtime/src/rpc/session.test.ts`, added by commit `eac52f0087`). The same package tests ordered application of shell and thread events (`state/shell-sync.test.ts`, `state/shellReducer.test.ts`, `state/threads-sync.test.ts`, `state/threadHistoryController.test.ts`). User-visible consequence in the reference: a sidebar row, a status line or a transcript never stays at an old value because a reply was swallowed. There is no reference analogue of "a reply for an answer not in flight".

### What exact2 does today

From `EXACT2-GAPS.md` section X14 (written by earlier sessions from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`; its citations as given there):

- "Without the change, a re-read with equal arguments could forget a live `native.later` call and drop its reply."
- "On main, `__exact_forget` still drops a let-go answer's pending native fetches unless another answer claimed them (`js/src/prelude.js:731-735,893-915`). Awaiting another answer's promise is fixed for Hermes main placement (`f96641ddd`). LLP 1097 is accepted but not built and covers storage only."
- "LLP 1097 (accepted; implementation 2026-10-07..09) changes the same code."
- Support needed there: "Keep a let-go answer's in-flight native request and hand its reply to the next answer for the same key, or let a module mark it as background work."

Bundled library (`20261005-platforms-v3`, topic state-and-data): it says a mutation or resource reads as its declared type and that a failed request keeps its previous value, but it does not cover `native.later` ownership: **not covered, unknown**. Nothing here has been reproduced on the pinned `main` by this plan.

### Where the clone hits it

Observed by the earlier session (lane `r3-protocol`; recorded in clone code, not re-run by this plan):

- `modules/apple/T3ReadGate.swift` (148 lines) header: "asking `snapshot` again while its previous answer still awaits a native reply drops that reply ("a reply for an answer not in flight") and the data resource keeps its old value." The gate holds the topics `t3.status`, `t3.events` and `t3.fleet` while a snapshot read is in flight (idle release 1.0 s, hard limit 20 s, settle 0.04 s, quiet cap 0.12 s), then replays them once. Cases are in `macos/tests/transport/r3.swift` (from line 108).
- `r3-protocol-reader.ts` (`beginRead`, `traceRpc`, `statusTicket`, `settleTraces`): every read request is tagged with a reader id and ends with `readEnd`; each RPC carries a trace id so that a call whose reply was dropped is still acknowledged, otherwise "Some requests are slow" would count it. `client.ts` `refresh()` starts every read through `beginRead` (about line 300).
- `r6-pr-actions.ts` `readDetail`: shares resolved values only, "a superseded answer's replies never arrive" (its comment; README "Framework limits worked around").
- `20261005-managed-codex-chatgpt` avoids a long parked reply with a start/take pair (`providerAuthStart`, `providerAuthTake`) because a parked reply can be lost.
- The round-12 failure F4 (false "Some requests are slow" toast, `20261005-round12-wrapup`) may be a symptom of the same drop. Not established.

What a person sees differently from the reference: updates that come from native topics (status, events, fleet) are coalesced and wait for the running read, by up to the quiet window after each read, and up to the idle release when a read stops making requests. A missed tag would show a stale list with no error. The reference has neither delay.

### The framework change that exists (context only)

An earlier session made a framework edit that was never filed upstream. It is in the local commit `a9f9e58ec` ("Oct 3 clone snapshot"; kept on `daehyeon/t3-code-oct3` by `20261005-clone-on-exact2-main`). This plan reads it only through `git show --stat` and the printed diff of `js/src/parking.rs`; it did not open `js/src/lib.rs` or the tests.

- `git show --stat a9f9e58ec -- js`: `js/src/lib.rs` (124 lines changed), `js/src/parking.rs` (new, 88 lines), `js/tests/fixtures/storage.ts` (+6), `js/tests/it/storage.rs` (+231); 372 insertions, 77 deletions.
- `parking.rs` adds an `impl Module` block that owns calls whose answers outlive one runner turn: `park(key, call, ticket, request)` records a call as `Parked { call, ticket, work_taken, staged }` under a key of `(target, source, argument bytes)` (in `streams` for a stream request, else in `parked`); `forget_unheld(in_flight)` keeps a parked call only while the runner still lists an in-flight answer with the same key (and the same continuation token when it has one) and forgets all others in the prelude through `__exact_forget`; `discard_parked`, `take_request`, `turn_open`, `deferred_work` and `key` are helpers. In short, a native request is tied to its key, not to the first answer that asked it.
- The commit message calls it "a framework edit pending its own PR". **The plan files no framework PR.** This issue may offer that change as a proposed patch, after `issue-open` has reproduced the problem on the pinned `main` and the user approves.

## Why it must be resolved

Parity goal: the clone must never show a value older than the one the server sent. Today that depends on a 148-line native gate plus a request-tagging convention that every new data read in the app has to follow (many planned tickets add reads: pull requests, providers, terminals, usage, automations). A forgotten tag fails silently. The gate adds latency the reference does not have, and it forces long native waits (provider sign-in, a loopback listener that waits up to 300 s according to the plan's research) into a start/take pair. Waiting tickets and rows: `20261005-clone-on-exact2-main` (acceptance row "X14 decision"; scope item 6 reproduces it on the pin), and `20261005-managed-codex-chatgpt` (start/take stays until this is resolved). Also open: the cost of the proposed patch overlapping LLP 1097, which "changes the same code" (`EXACT2-GAPS.md`).

## Requested support

Web analogue: a `fetch()` promise is not cancelled because the component that started it re-rendered; its result reaches whoever awaits it, and an equal request in flight can be shared. For a resource whose data source is asked again with equal arguments while its native request is in flight:

- **A. Keep the request (preferred).** Key the native request by (source, arguments) as the existing patch does. A re-ask with an equal key adopts the in-flight request. A re-ask with different arguments forgets the old request, and its late reply is discarded. The last answer to let go forgets the request.
- **B. Background work.** Let a module mark a native call as not tied to an answer's lifetime (its reply is delivered to a topic or to the next answer for the key). Smaller change to forget semantics, larger API.
- **C. Keep the app-side gate.** No framework change; the issue is then closed by a user decision and the gate stays as the documented pattern.

On the macOS host first (the only host with the native module). Other hosts: to confirm at `issue-open`.

## How to reproduce

To confirm on the pinned `main` at `issue-open` (not run by this plan):

1. Minimal app: a root resource `snapshot = read(tick)` whose data source awaits `native.later({op: "slow", ms: 500})` and watches a topic `x`; the module announces `x` 100 ms after the first ask.
2. Expected (reference-like): after 500 ms the resource shows the reply. Actual (per the clone docs): Exact asks again on the topic change, the first reply is dropped ("a reply for an answer not in flight"), and the resource keeps its old value until a later ask completes.
3. Clone scenario: build the clone with `T3ReadGate.held` emptied (local, uncommitted), pair a fixture backend (ports 16000–16999, isolated HOME and T3CODE_HOME), send turns while editing the draft (each edit announces `t3.events`); run the round-11 reply-ownership scenario named by `20261005-clone-on-exact2-main` (15 turns with concurrent draft edits; this plan did not find its text in the README). Expect a stale sidebar or transcript after a dropped reply.

## Acceptance for the fix

- The minimal app shows the reply after 500 ms (agent `state` and a screenshot).
- AppKit or runner test: a re-ask with equal arguments while the request is pending executes one native request and both answers see a value.
- A re-ask with different arguments discards the old reply; no value from the old arguments appears after the new one.
- After the last answer lets go, no native request stays pending (`state`) and the reply is discarded without a log error.
- The clone's gate tests (`macos/tests/transport/r3.swift`) still pass until the gate is removed; then the 15-turn scenario shows no stale value with the gate removed.
- Conformance or runner cases in the framework's own tests, if the upstream maintainers want them: to confirm at `issue-open`.

## App adoption after resolution

`issue-close` verifies: remove `modules/apple/T3ReadGate.swift` and its wiring in `T3Module.swift`; remove the reader tags and `readEnd` from `r3-protocol-reader.ts` and `client.ts` (keep trace ids only if the slow-request toast still needs them); let `readDetail` (`r6-pr-actions.ts`) share promises; let `20261005-managed-codex-chatgpt` use one long reply instead of start/take (optional); delete the gate cases in `r3.swift`. Rows that must pass: the transport binary, the 15-turn fixture scenario, and no false "Some requests are slow" toast after switching environments (round-12 F4).

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval). `20261005-clone-on-exact2-main` records the reproduction here.
