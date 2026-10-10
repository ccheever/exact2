# Data modules have no timers: `setTimeout` and `Date.now` are refused

**Status:** Open
**Systems:** TypeScript data sources, runner, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/124

## Current scope

Timers remain refused. Keep this issue open for the prerequisite failure-parity repair in existing open PR #228: typed input must land and resource fail, including synchronous/queued sends. Review/finish that PR, not a duplicate implementation. Charlie required resolving that slice before retiring the timer umbrella.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
A TypeScript data module cannot wait or read the clock. `setTimeout`, `setInterval`, `AbortSignal.timeout()`, `Date.now()` and `performance.now()` are refused by design (LLP 1027.000). A source that waits 450 ms (a debounce) fails on the web and is refused on macOS. **This is a policy gap: a decision is needed before implementation.** Ask: decide whether data modules get a timer API on the runner's clock (amending LLP 1027.000 D1), and if so, build at least a one-shot `setTimeout` inside an answer that the agent's `clock +N` moves.

## Why it matters
A coding-agent desktop app has client logic written against timers. Examples: a 450 ms debounce before a pasted link resolves, a 10 s retry cooldown, a 5-minute dedupe of a usage probe, a 25 s activity report, a 300 s sign-in expiry, and live refresh every 5 minutes. Today apps must replace each timer with one of three stand-ins:
- A root `task … every(ms, action)` that passes the time to a source as an argument. The resource identity then changes with time, and a debounce becomes a polling task.
- A native-module sleep for a wait inside a command.
- A timer in the host language (Swift). The agent's `clock +N` does not move it, so tests must wait real time.
Ported logic stops being the original code, and its fake-timer tests cannot be ported. LLP 1092's gated tasks (`task NAME when COND key=EXPR`) cover some debounce and window cases in Contract, but not a wait inside a source call.

## Current behavior (exact2 4c893fef6)
- `docs/reference.md:386-388`: "Not in a data module, by design (LLP 1027.000): timers (`setTimeout`, `setInterval`), `performance.now()`, `Date.now()`, `new Date()` without a value and `Math.random()`: time and seeds are source arguments." Line 381: "`AbortSignal.timeout()` refuses: no timers".
- Web (JS target): typing a query runs the source, which is refused: `refused reply resolved … setTimeout() is unavailable in data sources: there are no timers; pass time as an argument`. The resource keeps its placeholder and `failed(resolved)` is true.
- macOS: the whole input is refused: `input view 2 (edit) refused: Data { resource: "resolved", error: Unavailable("setTimeout() is unavailable …") }`. `tree` shows the field's value as `""` (`TextInput#2 [link] value=""`).
- `rules/DEFERRED.md` (Motion, LLP 1092 expansion) keeps "component-scoped and action-started timers, computed intervals" out.

## Expected behavior
Web standard (Chrome): `setTimeout(fn, 450)` in a module runs `fn` about 450 ms later; `Date.now()` reads the clock. **Proposal** (needs the decision above):
- `setTimeout`/`clearTimeout` inside an answer. The wait is the runner's, on its clock: `clock +N` moves it under the agent, and the OS clock moves it on a device.
- A timer belongs to the answer that made it. A newer request for the same resource forgets the older one, which gives a debounce.
- `Date.now()` and `performance.now()` agree with `exactTime.epochAtZero + now()`.
- Later, if decided: `setInterval`, `AbortSignal.timeout(ms)`.
- Hosts: web (both targets), macOS, iOS, Linux; the same refusal or the same behavior on each.

## Reproduction
Minimal app:
```contract
shape Reply
  text: string

component X19SourceTimers
  state query = ""
  resource resolved = resolve(query) as shape Reply else empty(text="idle")
  action edit(v: string)
    query = v
  view
    main testId="root" padding=24
      input value=query input=edit aria-label="Link" testId="link"
      text resolved.text testId="out"
      text (failed(resolved) ? "failed" : "ok") testId="status"
```
```ts
// app.ts; the import, appId and grants lines are as `exact new` writes them (full file in evidence/X19/app/)
const wait = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
export const answer: Answer = async (source, [query]) => {
  if (query === '') return { text: 'idle' };
  await wait(450); // debounce: a newer query replaces this request
  return { text: `resolved ${query} at ${Date.now()}` };
};
```
| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x19-source-timers`; files above; `contract build --json` | compiler | `[]` | `[]` |
| 2 | `agent web "type link a/b#1" "clock +500" "clock settle" tree logs` | web (Chrome) | `out` = "idle", `status` = "failed"; log: `setTimeout() is unavailable in data sources` | `out` = "resolved a/b#1 at …" after 450 ms |
| 3 | `bun exact.mjs mac`; same drive with `agent macos` | macOS 26 | input refused; `query` stays ""; `out` = "idle" | as on the web |

### Evidence
The files are attached at the end of this issue, in folded sections.
Quoted from `repro.log`:
```
web    Text#3 [out] "idle"   Text#4 [status] "failed"
web    t=0 refused reply resolved; wall 0 ms: setTimeout() is unavailable in data sources: there are no timers; pass time as an argument
macOS  TextInput#2 [link] value=""   Text#3 [out] "idle"   Text#4 [status] "ok"
macOS  t=0 input view 2 (edit) refused: Data { resource: "resolved", error: Unavailable("setTimeout() is unavailable in data sources: there are no timers; pass time as an argument") }
```
- `evidence/X19/repro.log`: commands, `tree` and `logs` on both hosts, and the quoted policy text.
- `evidence/X19/app/`: the minimal app source.

## Acceptance criteria
- A recorded decision (LLP 1027.000 amendment and a `rules/DEFERRED.md` entry, or a stated refusal with the supported alternative).
- If built: the minimal app shows "resolved a/b#1 …" after `clock +450`, not before, on web, macOS, iOS and Linux.
- Typing twice within 450 ms answers once, for the last query.
- `clock settle` does not hang on a pending wait; `state.pending` shows it.
- Under the agent, `Date.now()` in a source equals `exactTime.epochAtZero + now()`.

## Notes
- Policy: LLP 1027.000 D1 ("A time-dependent source takes the time it needs as a numeric argument … There is no … timer binding") and its section "Alternative: runner-backed ambient APIs", which says to choose that design "when a named library or application makes its ergonomic benefit worth that machinery". This issue names such an application.
- Workaround limits are listed under "Why it matters".
- Host difference seen here: the web lands the input and fails the resource; macOS refuses the input. That difference may deserve its own fix whatever the decision is.
- Not tested: iOS, Linux, the wasm web target; `Date.now()` alone (the source failed at `setTimeout` first).

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef65f1bf8d6e4be6df9265e2ae49790637 (2026-10-05), checkout exact2-repro, macOS 26.6.2

$ bun exact.mjs contract build app.contract --json
[]

$ bun exact.mjs agent web "type link a/b#1" "clock +500" "clock settle" tree logs
{"at":[111.5,33],"typed":2,"target":"link","delivery":"platform","carrier":"web","mode":"agent","clock":0,"epoch":4,"incarnation":1}
{"clock":500}
{"clock":500,"settled":true}
epoch 4 · incarnation 1 · clock 500 ms · 4 nodes
View#1 [root]
  TextInput#2 [link] value="a/b#1" label="Link" [focused] (input)
  Text#3 [out] "idle"
  Text#4 [status] "failed"
t=0 boot: 4 nodes, epoch 1
t=0 reply 1; wall 1 ms
t=0 reply 2; wall 0 ms
t=0 refused reply resolved; wall 0 ms: setTimeout() is unavailable in data sources: there are no timers; pass time as an argument
t=0 request 2 (resolved) failed and is no longer pending: it keeps its last value

$ bun exact.mjs mac   # cargo 107.7 s, swift 13.9 s
$ bun exact.mjs agent macos "type link a/b#1" "clock +500" "clock settle" tree logs
{"typed":2,"value":"a/b#1","target":"link","delivery":"platform","carrier":"macos","mode":"agent","epoch":1,"incarnation":1,"clock":0}
{"incarnation":1,"clock":500,"epoch":1}
{"clock":500,"epoch":1,"incarnation":1,"settled":true}
epoch 1 · incarnation 1 · clock 500 ms · 4 nodes
View#1 [root]
  TextInput#2 [link] value="" label="Link" [focused] (input)
  Text#3 [out] "idle"
  Text#4 [status] "ok"
t=0 boot: 4 nodes, epoch 1
t=0 resolved shows its build-time answer until its source answers
t=0 query resolved: resolve
t=0 resolved answered: equal to its build-time answer
t=0 data_ready (1 asked again) → epoch 1 (+0 −0 ~0)
t=0 query resolved: resolve
t=0 input view 2 (edit) refused: Data { resource: "resolved", error: Unavailable("setTimeout() is unavailable in data sources: there are no timers; pass time as an argument") }
  app: exact: Data { resource: "resolved", error: Unavailable("setTimeout() is unavailable in data sources: there are no timers; pass time as an argument") }

# docs/reference.md:386-388 at this revision:
Not in a data module, by design (LLP 1027.000): timers (`setTimeout`,
`setInterval`), `performance.now()`, `Date.now()`, `new Date()` without a value
and `Math.random()`: time and seeds are source arguments. Every executor
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
// X19: a data source waits 450 ms (a debounce) with setTimeout.
shape Reply
  text: string

component X19SourceTimers
  state query = ""
  resource resolved = resolve(query) as shape Reply else empty(text="idle")
  action edit(v: string)
    query = v
  view
    main testId="root" padding=24
      input value=query input=edit aria-label="Link" testId="link"
      text resolved.text testId="out"
      text (failed(resolved) ? "failed" : "ok") testId="status"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X19 Source Timers",
  "short_name": "X19 Source Timers",
  "id": "com.example.x19-source-timers",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x19-source-timers",
    "name": "X19 Source Timers"
  },
  "host": {
    "ios": {
      "minimumOS": "17.0",
      "deviceFamily": [
        "iphone",
        "ipad"
      ]
    },
    "macos": {
      "minimumOS": "14.0",
      "window": {
        "width": 900,
        "height": 700
      }
    },
    "web": {}
  },
  "deploy": {
    "store": {
      "web": "0",
      "macos": "0",
      "ios": "0",
      "linux": "0"
    }
  }
}
````

`app.test.contract`

````contract
test "a debounced source answers after 450 ms"
  type "link" "a/b#1"
  clock +500
  expect text "out" == "resolved a/b#1"
````

`app.ts`

````ts
import type { Answer, Result } from './app.contract.d.ts';

export const appId = 'com.example.x19-source-timers';
export const grants = '';
const wait = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
export const answer: Answer = async (source, [query]) => {
  if (query === '') return { text: 'idle' } satisfies Result<'resolve'>;
  await wait(450); // debounce: a newer query replaces this request
  return { text: `resolved ${query} at ${Date.now()}` };
};
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T08:38:05Z

## Decision needed
**[Design]**: an accepted LLP decides otherwise, or the API has to be chosen first. It needs a ruling before implementation. Re-checked on main `78286adc1` (2026-10-07). Partly changed: since 8be2b2623 a direct `setTimeout`/`Date.now()` fails the build on every host, by file and line. A call through an alias still passes the build.

**Blocked by:** LLP 1027.000 D1 (`llp/1027.000-explicit-time-and-randomness.rfc.md:60-62`: no timer binding; time is a source argument). The LLP itself names the condition for revisiting: "when a named library or application makes its ergonomic benefit worth that machinery".

**Options:**
- **A.** Amend D1: `setTimeout`/`clearTimeout` inside an answer, on the runner's clock (`clock +N` moves it). A newer request forgets an older answer's timers, which gives a debounce. `Date.now() = exactTime.epochAtZero + now()`.
- **B.** Keep the refusal.

**Recommendation:** B for now. The T3 clone moved its timed polls and its 250 ms PR-search debounce to gated tasks (LLP 1092) in #218, and they match T3 Code's timing. That shows the existing forms cover the consumer's cases. Revisit if a ported library needs timers inside a source call.

**In review:** #228 makes macOS and iOS treat a refused data-source answer as the web does: the input lands, the resource fails (`failed()` is true), and the log names the refusal. Before, macOS refused the whole input. The build also catches `globalThis['setTimeout']`-style lookups by literal name. #228 changes a recorded rule, so it needs a ruling too: LLP 1071's 2026-09-29 text says a source's own refusal ("unknown source, bad arguments, unavailable") refuses the commit on both runners. #228 makes a TypeScript answer's refusal fail the resource instead, and amends LLP 1027.000 D3. The other way to make the hosts agree, refusing the commit on the web too, isn't possible for an async answer, because the web learns of the failure only after the commit.

### ccheever — 2026-10-08T08:07:54Z

**Decision: Keep module timers refused; finish failure-parity repair separately.**

Keep the timer API deferred; leave this issue open while the separate failure-parity work is resolved.

The current consumer now uses gated tasks successfully. Preserve explicit time and the agent clock rather than adding a second timer lifetime inside answers. Charlie confirmed this deferral on 2026-10-08.

Keep open until #228's parity slice is resolved, then close the timer request not planned. Prefer failed resource/unsent mutation over losing typed input; require the JS synchronous-send and queued-send cases be covered, not silently left behind.
