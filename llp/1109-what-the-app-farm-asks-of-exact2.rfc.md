# LLP 1109: What the app farm asks of Exact2 — the decisions left after round 1

**Type:** RFC (a decision brief: each item proposes, Charlie decides)
**Status:** Accepted, 2026-10-08: every question ruled by Charlie (§R). D1 is implemented (`9731c8056`); D3 is implemented (`513450bda`); D2 is "no change".
**Systems:** Contract's `now()` (the roster, the runner's clock and every host's clock origin, the JS target's runtime, the Lean semantics and `contract-difftest`); the data seam's answer check (`host/web-js/ts-data.js`, `js/src/lib.rs`, `js/web/src/lib.rs`, the runner's `conform`); `failed(x)`; the agent driver and authored tests (`scripts/agent.mjs`, `scripts/agent-test.mjs`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-08
**Implementer:** each accepted item has its own lane. D1: Claude (Opus 5.5), `9731c8056`, 2026-10-08. D3: Claude (Opus 5.5), `513450bda`, 2026-10-08.
**Related:** LLP 1027.000 D2 (no wall-clock provider); LLP 1027.000.000 D2–D3 (the date as a host fact, `exactTime`; "start the agent clock at a real epoch" rejected there); LLP 1012 §2 and `rules/DEFERRED.md` §Agent API (the seekable clock; `clock` replaces `wait`); LLP 0382 (fail closed loudly in debug); LLP 1102 (the authoring bench's brief, whose shape this copies); the app farm's round-1 synthesis, `~/appfarm/synthesis/round-01.md` §2.2, and the build diaries under `~/appfarm/apps/<run>/DIARY.md`

## R. Rulings (Charlie, 2026-10-08)

- **D1: (c), and the name is `performanceNow()`.** `now()` is deleted,
  not deprecated; writing it is a compile error whose message gives both
  repairs in one sentence: `performanceNow()` for durations and timers,
  `time.epochAtZero + performanceNow()` for the date. Every app, test,
  conformance case, the Lean semantics, difftest's corpus, the docs and
  `exact new`'s templates move to the new name. The name was chosen after
  asking GPT-6 Astra (xhigh) and Grok 4.7 (xhigh) blind, with one brief:
  both picked `performanceNow()` over `elapsed()` (88% and 72%), because
  it is `performance.now()`, which agents already know as milliseconds
  from a time origin and not a date, and `time.epochAtZero +
  performanceNow()` mirrors the web's `performance.timeOrigin +
  performance.now()`; both refused the abbreviation `perfNow()` (the
  alias rule). Grok's caveat stands: the rename shrinks the trap without
  removing it, so the refusal's date form is required.
  *Implemented in `9731c8056`:* the roster entry is `performanceNow` (ordinal 0,
  `Stdlib::PerformanceNow`, the JS target's `x_performanceNow`); an
  unresolved `now(…)` is `type-now-renamed` at the call, and `Date.now()` and
  `performance.now()` written as methods meet the same sentence; an app's own
  `fn now` still shadows. Every app, test, conformance case, the Lean
  semantics, difftest's corpus and generator and the docs moved; the pitfall
  about 1970 deadlines went, per its file's rule. Earlier LLPs (1096,
  1027.000.000 and others) keep `now()` as written: read it as
  `performanceNow()`.
- **D2: (a).** Keep refusing, loudly, as `0ddec5d6b` does; revisit with
  round 2's numbers.
- **D3: add `failure(x)`, answering `option<{code, message}>`**: `none`
  while `x` has not failed, else a stable `code` from a small closed
  vocabulary (an app branches on it and it is the same on every host)
  and the `message` `state.failed` shows (for a developer, or an app
  that chooses to show it). The vocabulary is fixed by the implementing
  lane from the runner's actual failure classes and documented in the
  grammar. `failed(x)` stays.
  *Implemented in `513450bda`*: six codes, `offline` (a request's
  `Network` failure let through), `timeout`, `refused` (outside the
  grants), `shape` (an answer outside its shape), `storage` (a coded
  storage refusal let through) and `error` (everything else, a source's
  own error among them); `runner/src/failure.rs` and
  `docs/contract-grammar.md` hold the table. A response over its size
  limit is `refused` on every host (2026-10-08: it was `offline` on the
  web's and Apple's plain HTTP, `refused` on Apple's event stream; the
  server answered, so `offline`'s "reached no server" is wrong, and a
  retry gets the same answer).

## 0. Summary

The app farm built about 270 small web apps from `exact new`, each on a
Snapback 4 backend, with two builder models (`gpt-6.1-sol`, "GPT", and
`muse-spark-1.3-contributor`, "Muse"); round 1's synthesis parsed 109
finished builds. Its Exact2 clusters, worst first, were: `now()` (12 builds,
the only Exact2 blocker), a resource answer with an extra field failing
silently (13), Contract words that differ from HTML and JS (26), `secret.keep`
lost between drives (2), replies that land only at `clock settle` (20),
concurrent builds (13), and Chrome's allocator line (8).

What needed no decision has landed:

| Cluster | Landed |
| --- | --- |
| Chrome's allocator line | `aef79f76f`: browser noise, filtered |
| `secret.keep` lost between named drives | `b5960b938`: a named store's Chrome quits gracefully, so `localStorage` is written |
| Concurrent `web`/`test`/`agent` builds | `6ddd0c07f`: each JS-target build stages privately and swaps in under a lock |
| Contract words | `ff62431f7`: repairs for `heading`, `table`, `form`, `details`, `count`/`size`, `else []`, `else Shape(…)`, and more |
| A silent shape refusal; an early `tree` | `0ddec5d6b`: `state.failed`, a failing `expect` and a CLI drive name the failed resource and why; a read before the clock moves with requests in flight says so |
| `now()` | the pitfall, the grammar row and the agents' guide say it is `performance.now()`, not `Date.now()` (this commit) |

Three questions remain, each a semantic or a default the rules care about.

## D1. `now()` is the date

**What happened.** Under the agent and in tests, an app computed
`closesAt = now() + 7 * 86400000` and sent it to its backend: `604800000`,
1970. The backend's expiry job swept the row while the mutation reported
`sent` (asthma-action-plans-0661); a label read "due in 20732d"
(invoice-chaser-0415); budget-tokens-0900 wrote a 1970 deadline into its
persisted store and closed its vote (the blocker). 11 of the 12 builds were
Muse's; GPT read `exactTime().epochAtZero` or used server time. The
builders who went looking found the grammar row that says `now()` is "not
a date", but only after the bug (worm-bin-monitor-0896,
asthma-action-plans-0661). The workaround everywhere was a server-computed
deadline.

**Why docs are not enough.** The trap is the name. `now()` reads as
JavaScript's `Date.now()`, which every builder has seen millions of times;
it behaves as `performance.now()`. This is not only a test artifact: `now()`
is elapsed time on every host (`performance.now() - t0` on the web,
milliseconds since `main` on Apple), so a shipped app has the same bug
seconds after launch instead of at zero. The farm found it under the agent
only because the agent's clock starts at exactly 0. The rule is that a
semantic which could follow the web follows the web (`rules/RULES.md`
§Scope), and the web's `now` that an author reaches for is the date.

**Options.**

- (a) **Keep `now()` elapsed; documentation only.** Done (§0). It costs nothing
  and leaves the name as the trap.
- (b) **`now()` is Unix milliseconds on every host**: `epochAtZero + elapsed`,
  read from the same monotonic clock as today, so it never steps backwards
  within a session. Under the agent the epoch is the drive's (`--epoch`, a
  test's `epoch` line, default `2026-01-01T00:00:00Z` as now), so a drive stays
  deterministic and `clock +N` still moves it. `state.clock`, `clock <ms>`,
  timers (`after`, `every`) and transitions stay elapsed: none of them reads
  `now()`'s value. What changes: `playSound(at=)` is "on `now()`'s
  milliseconds" and moves with it; an app that compared `now()` with a
  hard-coded elapsed value; the Lean semantics' `now` and difftest's corpus;
  any fixture or test that prints `now()`. `exactTime().epochAtZero` stays
  for code that wants the boot instant. This reverses LLP 1027.000.000 D3's
  "rejected: start the agent clock at a real epoch", whose reason was that it
  "breaks every check that reads `clock`"; (b) leaves `clock` elapsed and
  moves only what `now()` returns.
- (c) **Rename `now()` to `elapsed()`** (delete, don't deprecate), with a
  refusal for `now()` that names `elapsed()` and `time.epochAtZero +
  elapsed()`. The trap becomes a compile error with its repair; the author
  still needs the `exactTime` resource for the date.

**Recommendation: (b).** It gives the authors what they wrote; (c) only
tells them they wrote the wrong thing. If (b)'s audit of `now()` readers
turns out larger than a lane, (c) is the cheaper half-step.

## D2. An answer with a field its shape lacks: refuse, or drop

**What happened.** An answer is held to its shape exactly on every executor
(`docs/reference.md`), so a backend row with one extra column (`orgId`,
`createdAt`, `approver`) failed its resource. Until `0ddec5d6b` the refusal
was only in `logs`: the view kept its placeholder or `[]`, `failed(x)` drove
an "unreachable" banner (pantry-pop-up-0583), and tests failed looking like
timing (roast-curve-notes-0088). 13 builds (7 GPT, 6 Muse) lost a median of
2 minutes and up to 15 (recital-slots-0157). The usual cause was a spread,
`{ ...row }`, which TypeScript's structural typing admits and the generated
`app.contract.d.ts` cannot refuse.

But GPT builders praised the strictness 9 times in their "Lean in" sections:
"Strict runtime shapes caught an accidental backend-field leak"
(shade-seat-diary-0993). LLP 0382 says a fail-closed refusal must name itself
in a diagnostic build; the silence broke that, not the strictness.

**Options.**

- (a) **Keep refusing.** It is now loud where the author looks: `state.failed`,
  the failing `expect`, the drive's stderr.
- (b) **Drop undeclared fields silently**, as the web does: JSON has no
  schema, and TypeScript's types are structural.
- (c) **Drop and say so once** per source and field in a development build:
  a journal line and `state.failed`'s neighbour, `state.dropped`. Missing
  fields and wrong kinds stay refusals.
- (d) **A shape opts in**: `shape Row open` admits extra fields, dropped.

**Recommendation: (a) now, revisit with round 2.** The loud refusal removes
the cost the farm measured without giving up what GPT valued. If round 2
still shows more than a few builds losing time to it, take (c): it keeps the
leak visible and stops failing the view.

## D3. A failure's reason in the view

`failed(x)` is a boolean, so an app's one failure banner says
"unreachable" for a shape mismatch, a refused grant or a server 500 alike
(pantry-pop-up-0583). `rules/DEFERRED.md` rules out a development overlay
("no devtools UI"). The option is a roster function, `failure(x)`:
`option<string>`, the reason `state.failed` shows, for an app to display or
branch on. **Recommendation: not yet.** `state.failed` serves the author while
building; a reason in the view is for the app's users, and the farm has no
evidence that apps need it.

## Considered, not proposed: drives that wait for data

The synthesis suggested that `agent` operations land in-flight requests
before each read. `rules/DEFERRED.md` §Agent API makes `clock` replace a
`wait` on purpose, and authored tests already land the app's data before
their first step. Instead, `0ddec5d6b` makes a read taken before the clock
first moves, with requests in flight, say so on stderr and name
`clock data`.

## What round 1 asks that this brief leaves out

The other Exact2 clusters, which are not decisions (file pickers under the
agent, `Result<'…'>` keyed by source name, refused paint on native
controls, data-module globals refused late, layout defaults, string
children, mutation answer lifetime, the first-open `ENOENT`, test-grammar
papercuts, one fresh session per `agent web`), stay with the synthesis; the
seam clusters (Snapback 4 from a data module) are another lane's.
