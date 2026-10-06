# LLP 1103: Making a fetch fail in a test or a drive

**Type:** RFC
**Status:** Built r4, 2026-10-06 (§7). Charlie accepted the direction (LLP 1102 §0, §3.3). r1 was reviewed blind by Astra (`gpt-6-astra`, xhigh) and Grok 4.7 (xhigh); both found it not ready (§6). r2 narrowed the first version to what every executor can do exactly. r3 folded Astra's pass on r2 (NOT READY on five specification gaps, all answered).
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

**Where.** The table is consulted at the one point on each path that holds the full URL before anything goes out. That is not always the grant check itself: native admission happens in ibex and sees only an origin. The points are:
- **Hermes and native Rust requests:** the host side of the request, where the runner's host request is turned into a transport call (the caller of ibex's admission, which still holds the URL). A module worker's requests return through this same path.
- **The JS target:** `fetchWith` (`host/web-js/admission.js`), before `admitsNetwork`. Web workers yield their requests to the page, so they reach it too.
- **The wasm web host:** `fetchEarly` (`host/web/module-glue.js`) as well as the later host request. `fetchEarly` starts a permitted GET before the runner's request claims it. So the early start consults the table, and a matched GET is not started early. The later request is then the one that consumes the fault and fails. A request is counted once, at whichever point decides it.

A fetch whose URL starts with an armed prefix does not go out; it fails at that point.

Matching by URL is also what the builders reached for (CDP's `setBlockedURLs` is URL-shaped), and it is executor-agnostic.

### D2 — The failure is each executor's real network failure

A matched fetch fails through the same path a refused connection takes on that executor:
- **Hermes:** the `fetch` promise rejects with the prelude's `FetchError`;
- **the JS target:** `fetch` rejects with `FetchError('Network', …)`, as `fetchWith` wraps a real browser failure (`host/web-js/admission.js`);
- **a Rust source's declarative request:** its outcome is `Outcome::Failed { kind: Network }`.

No new error shape is invented. A source that catches the failure and returns a record lands as it would after a real failure. One that throws sets `failed(resource)` as it would.

The journal records each injected failure ("fetch failed (driver fault): https://…/recipes/3"), so a test author can see that the fault fired.

### D3 — Arming

- **A launch line in a test,** before the first step, as `locale` and `epoch` are: `fail fetch "…"`. It is armed before the app boots, so the first data load fails. This covers the most common case, "the API is down when the screen opens".
- **A step later in a test,** affecting every later matching fetch: `fail fetch "…"`, `pass fetch "…"`. With `times N` it fails the next N matching fetches, then passes.
- **A drive:** `--fail-fetch <prefix>` at open (repeatable), and `"fail fetch <prefix>"` and `"pass fetch <prefix>"` as operations on every carrier.

An armed fault lasts until `pass`, its count runs out, or the test or drive ends. A fault with a count that never fired is a test failure ("fail fetch … times 1 matched no fetch"), so a test cannot pass by testing nothing.

**The table.** There is one table per session (one app instance under the driver). It is shared by every source, placement and request lane. Its rules:
- each actual request consumes at most one count;
- when prefixes overlap, the longest matching prefix decides;
- arming a prefix again replaces its entry;
- `pass` removes an entry's matching but keeps its hit count.

The driver reads the table (entries, counts left, hits) from `state.faults`. That is how the unfired-fault check is made at the end of a test.

**Reload.** `reload` relaunches with the table as it is now, not as it was at launch. Today `scripts/agent-test.mjs` replays the original launch facts. With faults, that would bring back a fault that was cleared or used up. So the driver reads `state.faults` before relaunching and sends the current table as the launch facts.

**Grammar.** `fail` and `pass` are new step words. `hold` stays the drag modifier it already is.
- `fail fetch` lines that come before the first ordinary step are launch facts.
- The same line later in the test is a step. `launch_word` (`contract/syntax/src/parser/steps.rs`) classifies by position for this word, not unconditionally.
- Several prefixes may be armed.
- `times N` takes a positive integer.
- File-level launch inheritance (`contract/cli/src/lib.rs`) merges `fail fetch` facts by prefix, not by variant.
- The AST and its JSON form carry the prefix and the count.

**Transport.** Web launch facts go in the page URL before navigation, so they are added to the navigation parameter allowlist (`host/web/navigation.js`). Native facts go in the launch environment, on a simulator and a device too. Faults are installed before data activation. A release build ignores them: the facts are development-only, like the agent socket.

### D4 — Interaction with the runner's request handling

A failed fetch is an ordinary failed request:
- **Deduplication** and **refresh** see a request that ended, as with a real failure. Nothing is left in flight.
- **`clock settle`** and **`clock data`** count it as settled once it has failed.
- **A queued mutation** whose request fails behaves as after a real failure: its reply lands, the queue moves on.

There are no tickets to manage and no held state to retire. That is the reason r2 defers holding (D5).

### D5 — Deferred: holding a request, delays, storage faults

- **`hold`** (a request that stays in flight until released) would make a timeout testable, but the reviews found it interacts with deduplication, `forced` refreshes, `clock settle`'s device-hold rule and session teardown in ways that need their own design. Until then, a timeout is tested against a stand-in that never answers, as the bench's t8-library does.
- **Storage faults** on Hermes start the adapter operation before the runner sees the continuation, so they need a separate interception point. Storage failures are rarer in the bench.
- **Streams.** A WebSocket or `exactStream` open has its own admission and transport, and its failure reaches the stream's event mapper as `{type: 'error', kind: 'Network'}`, not a rejected fetch. It needs its own interception and tests, so it is deferred.
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
| The session's fault table and its consultation on each path (the native host request, `fetchWith`, the wasm host's `fetchEarly` and later request), with the executor's real failure, counted once per request | a lane-day and a half |
| Launch facts on each carrier (the URL allowlist, the native launch environment, development-only), `state.faults`, the drive operations, journal lines, reload carrying the current table | a lane-day |
| Test grammar (positional launch lines and steps, several prefixes, counts, inheritance by prefix, AST and JSON), the test runner, the unfired-fault failure | half a lane-day |
| Tests on each executor; docs (the guide's testing section, the grammar, an error-state recipe) | half a lane-day |

About three and a half lane-days.

## 5. Open questions

1. Prefix or glob? A prefix is enough for the bench's apps. CDP's patterns allow `*`. Proposed: prefix now, `*` later if asked.
2. Streams are deferred (D5): their opens go through separate admission and transport.

## 6. Revisions

- r1, 2026-10-06: first draft (fail and hold a source's host request, by source name).
- r2, 2026-10-06: Astra and Grok found r1 not ready:
  - source attribution does not survive `await`s, Hermes interleaving, Rust targets or workers;
  - native storage dispatches before the runner sees it;
  - a hold breaks against deduplication, refresh, `clock settle` and teardown;
  - the failure shapes were wrong (`FetchError`, not `TypeError`, on Hermes);
  - a fault could not be armed before the first data load.

  r2 matches by URL prefix at the grant check, uses each executor's real failure, arms on a launch line, and defers `hold` and storage faults.
- r3, 2026-10-06: Astra's pass on r2 (NOT READY) found these gaps, and r3 fills them:
  - the JS target's failure is `FetchError('Network')`, not a bare `TypeError`;
  - native admission sees only an origin, so the table sits where the URL is still held;
  - the wasm host's `fetchEarly` must consult the table, counting each request once;
  - one session table, with overlap, re-arm, `pass` and hit rules, read through `state.faults`;
  - `reload` relaunches with the current table;
  - the grammar is positional, takes several prefixes, merges by prefix, and needs AST and JSON support;
  - launch facts need the URL allowlist and are development-only;
  - streams are deferred.

  The cost is raised.
- r4, 2026-10-06: built (§7). Astra reviewed it blind (LAND WITH FIXES); Grok's pass ended without a verdict. Folded in:
  - an early GET sent before arming is not failed;
  - the unfired check fails a counted fault whose table never appeared, an unreadable state, and one about to `close`;
  - only a test's leading lines override the file's;
  - replacement runners carry the table;
  - `--fail-fetch` reaches `--test`;
  - the drive request is a form of `prefer`, so the operation count is unchanged;
  - the production claims are corrected.

  A second pass by both (LAND WITH FIXES) found four more, folded as the third and last round:
  - `reload` keeps a launch table the page has not made yet;
  - a window the app closes cannot hide an unfired counted fault;
  - the render host does not consult faults;
  - Linux routes `faults` by its top-level key.

  Grok's forced-GET note is not a defect: an uncached forced GET is a new request, which a fault may fail, and the early GET it does not claim is aborted.

## 7. As built

**The table.** It is one per session, in two places, because the two kinds of executor are two programs.
- **Native** (Apple, Linux): the runner holds the table (`runner/src/runner/faults.rs`). Each host's plain-request arm, where it would hand a request to the executor, asks `Runner::fault_dispatch` first. A match is counted there and becomes work that returns `Outcome::Failed { kind: Network }`. The shared native executor (`host/apple/src/executor_core.rs`) now runs a plain request's work instead of the transport, so the request never goes out.
  - A Hermes source's `fetch` rejects with the prelude's `FetchError('Network')`, as for a refused connection.
  - A module worker's requests take the same arm.
  - The render host has no driver and never consults it.
- **The web** (both targets): the page holds the table (`host/web/faults.js`, on `globalThis`, because the builds copy these modules). It is consulted at two points, after the grant check:
  - the wasm host's `request` (`http-body.js`) returns the browser-failure kind;
  - the JS target's `fetchWith` throws `FetchError('Network')`.

  `fetchEarly` only declines to start a matching GET, so each request is counted once. A GET `fetchEarly` already sent, before the fault was armed, is claimed and not failed.

**Rules.** The longest live prefix decides. Arming again replaces an entry. `pass` keeps its hits. A count runs out and the entry stays, with `left: 0`. Every injected failure writes the journal line "fetch failed (driver fault): <url>".

**Arming.**
- At launch:
  - natively, the runner reads `EXACT_AGENT_FAIL_FETCH` under `EXACT_AGENT=1` (simulators get it through `SIMCTL_CHILD_`);
  - on the web, the page reads `?failFetch=` under `?agent`, which the history allowlist carries.

  Both take one `<prefix>[\t<times>]` line per fault, and a reload's whole entry is `<prefix>\t<times>\t<left>\t<hits>\t<armed>`. A production build honors neither:
  - a native one drops every `EXACT_AGENT_*` variable before the host or the runner reads one (LLP 1069.007 D2);
  - a web bake compiles the reader out with `AGENT_ADMITTED`.
- During a drive, it is a form of `prefer`, not another operation: the network is the environment, as `online` is (LLP 1012 §1 keeps ten). On the wire it is `{"op":"prefer","faults":{"fail":…,"times":…}}`, `{…{"pass":…}}`, or `{…{}}` to read; the command line spells it `fail fetch …` and `pass fetch …`. Each carrier hands it to its table: the runner on native, the page on the web. `state.faults` lists the table when it is not empty, and `--fail-fetch <prefix>` arms one at open (and for every test, with `--test`).
- A replacement runner, from a development reload, carries the table as it is (`Carried::faults`). A JS-target development page reloads with its launch URL, as it carries no state.

**Tests.**
- `fail fetch` lines that lead a test (or sit at a file's top level) are launch facts; later ones are steps. Inheritance goes by prefix.
- `scripts/agent-test.mjs` fails a counted fault that matched no fetch (or whose table no fetch ever made), at the line that armed it: at the test's end, before it is armed again, or before a `close`.
- Before each input while counted faults are outstanding, the runner reads `state.faults` and drops those that have fired. A press the app answers with `close()` therefore cannot take an unfired fault's evidence with it: one still outstanding when the window closes fails the test.
- `reload` relaunches with `state.faults` as the launch table. A page whose table no fetch or fault request has made yet keeps its launch's, and an unreadable state is an error. On the web, the page reloads with the new `failFetch`.

**Verified.**
- A scratch app's four tests pass on the web JS target, the wasm web host, macOS and an iOS simulator:
  - a launch fault shows the error, `pass` and a retry load;
  - a counted fault fails once;
  - a later fault fails a retry;
  - a reload keeps a passed fault passed.
- On Linux, launch arming, `faults` and `state.faults` work; no app with a Linux presenter fetches at boot, so a hit is covered by the shared executor and the runner tests.
- Unit and integration tests: `runner` (the table), `contract/cli/tests/it/fetch_faults.rs` (runner dispatch, journal, state, streams not matched), `tests_decl.rs` (grammar, inheritance, refusals), `host/web/faults.test.mjs` (both web paths, `fetchEarly`, never sent).

**Known difference.** On the web the grant check comes first, so an ungranted URL is still `Refused`. Natively the fault is decided before the executor's admission, so a faulted ungranted URL fails as `Network`. Both are failures a test can see. Making native match would need a URL-level grant query the executor does not expose.

**Deferred, as D5 says:** holding a request, a staged status, storage faults, and streams.
