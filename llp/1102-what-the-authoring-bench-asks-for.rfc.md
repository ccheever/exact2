# LLP 1102: What the authoring bench asks for — the decisions left after the loop

**Type:** RFC (a decision brief: each item proposes, Charlie decides)
**Status:** Draft r1, 2026-10-06. Awaiting Charlie's decision on each item in §3. Not reviewed.
**Systems:** Contract (`contract/{syntax,types,analyze,lower}`), the roster (`plan/tables/format.json` `stdlib`), the runner, the JS target (`host/web-js`), the web host (`host/web/index.html`'s control reset), the Apple hosts, the kernel's length values, the agent driver (`scripts/agent*.mjs`) and the authored-test grammar, the data module's `storage`, the Lean semantics and difftest (for any roster change), and the authoring bench itself (`ccheever/authoring-bench`: graders, tasks)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Implementer:** none yet. Each item that Charlie accepts gets its own lane (and, where marked, its own RFC).
**Related:** LLP 1087 (the authoring bench; this is its §8.1 step 4, "what needs a human"); LLP 1088 (what the app diaries ask of Contract: D2 deferred numeric parsing with a trigger this bench has now met, §3.1); LLP 1054.000.003 (the formatters); LLP 1092 (gated tasks); LLP 1094 D8 (a drag during the last drop's session); LLP 1035.000 D9 (`autocomplete` at mount); LLP 1001 (`position: fixed` is not a row); LLP 1064 D6 and LLP 1069.001 (the web's control reset); the bench's findings registry, `analysis/findings.md` in `ccheever/authoring-bench`

## 1. Summary

From 2026-10-04 to 2026-10-06 the authoring bench ran 659 counted trials:

- 313 Claude trials on the web;
- 128 Claude trials on the web and iOS;
- 218 Codex (`gpt-6-astra`) trials on the web.

Each built one of seven small apps from a brief, kept a diary, and was graded by a scripted checker.

The loop fixed what exact2 alone could fix: land1 through land56 (diagnostics, docs, pitfalls, a driver timeout, the JS target's submit race, and more). In the last eight rounds almost every cell built and passed at 100%. Each round now yields about one small fix.

What is left is in this document. Each item needs a decision: a language or API addition, a change to a deliberate default, a test feature, or a change to the bench's own graders.

The short version of the recommendations:

| Do now (cheap, clear value) | Do with its own RFC | Docs only | Leave as is |
|---|---|---|---|
| `parseNumber`, `ceil`/`round`, a fixed-decimal format (§3.1, §3.2); `px` strings and `none` where CSS takes them (§3.10, §3.11); number-input bounds as numbers (§3.12); `autocomplete` (§3.6); a fail/hold-a-source drive op and test step (§3.3); the JS target's autofocus at mount (§3.19); silence fixes in the driver (§3.17); the two grader fixes (§4.1) | the default look of a bare input (§3.15); a settled-field write-back for text inputs (§3.16); CSS `min()`/`max()`/`clamp()` (§3.13); `storage.kv` (§3.8); ISO dates in the roster (§3.4) | fetch timeouts as a Contract pattern (§3.5); state that starts from a resource (§3.9) | `position: fixed` (§3.14); the stale-read check's strictness (§3.7) |

## 2. How to read the numbers

Each count is the number of distinct trials whose diary names the item in its "Rough", "Guesses" or "Needed" sections. Narrative and checkpoints are not counted.

The patterns were matched by a script (`count_items.py`, kept with the findings) and then read by hand for the top items. The counts are approximate: a diary can name a need without the word the pattern looks for, and the reverse.

Things to keep in mind:

- **A rate is per task that can need the item.** The bench's seven tasks are a tip calculator (t1), a to-do list (t2), a recipe browser with a failing API (t3), a kanban board (t4), a pomodoro timer with a settings field (t5), the to-do list extended (t6), and a sign-up wizard (t7). Number parsing can only come up in t1, t5 and t7, so its rate is out of those trials. Real apps will meet most of these items more often than the bench does, because the bench has only seven apps.
- **Minutes are self-reported** where a diary gave them. They are a floor: a builder who designed around a gap usually reported no time for it.
- **Claude's diaries are detailed; Codex's are terse.** Codex rarely names a missing feature; it works around it and says "none so far". A zero in the Codex column is not evidence the item does not hurt Codex.
- **The bench's 1087 rules apply to any change that follows.** Compiler, runner, kernel or host code needs a measured win or a correctness test; docs land unmeasured.

Claude trials per task: t1 65, t2 52, t3 69, t4 53, t5 70, t6 51, t7 72.

## 3. The items

Each item has the same parts:

- what builders hit;
- how often;
- what it costs them;
- what the fix would be and what it costs exact2;
- my recommendation.

Costs are rough. The yardstick is LLP 1088 stage 3 (`slice`, `replaceAll`, `toLowerCase`). That stage touched the roster, types, runner, JS target, Lean semantics, difftest and docs, and was one lane-day plus two review rounds.

### 3.1 Parsing text to a number — **do it**

**What builders hit.** A field's text has to become a number, to compute a tip, check that minutes are 1–60, or check that a team size is 2–50. Contract has no parse.

Builders worked around it two ways:
- a data-module source called as a resource on every keystroke;
- a literal list of the valid strings with `indexOf` (`indexOf(["1", …, "60"], trim(s)) + 1`).

**How often.** 130 of the 207 Claude t1, t5 and t7 trials (63%):
- t1: 36 of 65;
- t5: 52 of 70;
- t7: 42 of 72.

23 trials used the literal-list workaround. Median 5 minutes where reported (46 reports, 304 minutes). This is the most-mentioned item in the bench by a factor of two.

**What it costs them, beyond minutes.** The source round trip changes the app's shape: a pure calculation becomes a resource, an async answer, and on native a later turn. ios23 t5's tests passed on the web and failed on iOS because a validation answer landed a turn later there (land48's pitfall). Validation that should be synchronous became asynchronous.

**LLP 1088 deferred this** with a trigger: "a view that needs a number from text on each keystroke, with no domain meaning". The bench meets that trigger in nearly two thirds of the trials that can. 1088's concern was footguns: `Number("")` is 0, and `NaN` spreads. A parse that returns an option answers both.

**Proposal.** `parseNumber(text): option<number>`:
- a strict decimal grammar: optional sign, digits, an optional fraction, optional surrounding whitespace;
- no exponent, no hex, no `Infinity`;
- `none` for anything else, including `""`.

That is narrower than `Number()`, on purpose, and the same on every executor. `parseInt` and `Number` keep their `idioms.rs` refusals, which should then name `parseNumber`.

**Cost.** One roster function: plan table, types, runner, JS target with its budget, Lean, difftest, docs. About one lane-day. The grammar is small enough to state exactly in Lean.

**Recommendation.** Do it.

### 3.2 Fixed decimals, `ceil`, `round` — **do `ceil`/`round` now; a fixed-decimal format with care**

**What builders hit.** Showing `$12.34` needs two decimals. Rounding up a per-person share needs `ceil`. The roster has `floor`, `min` and `max` only. Builders wrote `0 - floor(0 - x)` for `ceil`, or moved money math into a source.

**How often.** 55 trials:
- t1: 41 of 65 (63%);
- t5: 10 of 70.

Median 2 minutes; the bigger cost is the same source round trip as §3.1.

**Proposal.** Two parts:

- **`ceil` and `round`.** Trivial, with one exact rule: `round` follows JavaScript's `Math.round` (ties toward +∞), so the web is the oracle.
- **A fixed-decimal format.** `formatNumber(n, "fixed", digits)` with `digits` a literal 0–20.

The fixed format needs one decision. JavaScript's `toFixed` rounds on the exact binary value and picks the larger candidate on a tie. Rust's `format!("{:.2}")` picks the even one. They differ only on exactly representable ties (`0.125` gives `"0.13"` in JavaScript and `"0.12"` in Rust). Follow the web: implement `toFixed`'s rule in the runner and test the ties.

A currency style (`formatNumber(n, "currency", "USD")`) belongs with the existing `format` capability (LLP 1054.000.003). It is locale-dependent, so it should wait for a consumer who needs more than `$` and two decimals.

**Cost.**
- `ceil`/`round`: an hour each across the stack.
- `"fixed"`: about half a lane-day, mostly the tie rule and its tests.

**Recommendation.**
- `ceil` and `round`: now.
- `"fixed"`: now, with `toFixed`'s rounding.
- Currency: later.

### 3.3 Making a source fail (or hang) in a test or drive — **do it, in the runner, on every host**

**What builders hit.** The recipe task asks for an error state and a retry. No authored test can make a fetch fail.

What builders did instead:
- read `scripts/agent.mjs` to find raw CDP (`s.carrier.call('Network.setBlockedURLs')`);
- wrote a fault proxy and pointed a copy of the app at it (a copy outside its folder also needs `exact.mjs update`);
- on iOS, rebuilt against a dead port.

**How often.** 44 of 69 t3 trials (64%), plus 7 Codex trials. Median 5 minutes; 207 minutes reported in 26 reports. ios24 t3 alone reported about 30 minutes.

**Proposal.** A driver op and a test step, answered by the runner's data seam:

```text
fail "recipes" ["message"]     # the source's next ask answers as a failure
hold "recipes"                 # the next ask stays in flight until released
release "recipes"
```

The runner already distinguishes a failed answer from a domain answer (`failed(resource)`). The agent already holds device requests (auth, file pickers; `holds()` in `host/web-js/agent.js`). This generalizes that to a named source, on every host. CDP blocking is web-only and URL-shaped; the seam is host-independent and source-shaped.

`hold` also makes a timeout testable (§3.5). A held request against a `task … when pending(r)` with `after(ms, giveUp)` is exactly the case ios21 t3 could not stage.

**Cost.** Runner (one injection point per answer path), JS target, the agent op on each carrier, the test grammar and runner, docs. Two to three lane-days. It is the largest of the "do now" items, but it would end the single largest time sink in the bench.

**Recommendation.** Do it, as a small RFC (its interaction with queued mutations and streams needs stating).

### 3.4 ISO dates and date arithmetic — **an `"iso"` style now; arithmetic stays in the data module**

**What builders hit.** "At least 13 years old" needs today's date as `YYYY-MM-DD` to compare with a date input's value.

Builders did one of two things:
- passed `exactTime().epochAtZero + now()` into a source that formats it;
- wrote civil-from-days arithmetic as about fifteen `fn`s.

**How often.** 27 of 72 t7 trials (38%). Median 10 minutes, the highest per-occurrence cost in this list.

**Proposal.** Add `formatDate(ms, offsetMinutes, "iso")`, giving `YYYY-MM-DD` in the given offset. It is deterministic and already fits `formatDate`'s shape.

"13 years before today" is calendar arithmetic (month lengths, leap days) and stays in the data module, as LLP 1088 argued for durations. Comparing `"2013-10-06" <= value` already works (1088 D1, string comparison).

**Cost.** Under half a lane-day.

**Recommendation.** Do the `"iso"` style; leave arithmetic in the data module.

### 3.5 A timeout on a fetch — **docs: the Contract pattern, plus §3.3 to test it**

**What builders hit.** A server that hangs shows "Loading…" forever. A data source has no timers, so there is no `setTimeout` and no `AbortSignal.timeout`.

**How often.** 32 of 69 t3 trials (46%). Median 3 minutes. Most builders noted it and moved on.

**Proposal.** The timeout already exists in Contract:

```text
task waiter when pending(recipes)
  after(10000, giveUp)
```

Write it up as the recipe for a timeout. With §3.3's `hold` it becomes testable.

A host-side `fetch(url, { exactTimeout })` would duplicate this in the data module, and would need an agent-clock story (a virtual timeout under the driver).

**Cost.** Docs: an hour.

**Recommendation.** Docs now. No data-module timeout unless a source-internal retry policy needs one.

### 3.6 `autocomplete` on inputs — **do it**

**What builders hit.** HTML's `autocomplete="email"` and friends are refused (`lower-unknown-attr`). A sign-up form loses autofill and password-manager hints.

**How often.**
- 38 of 72 t7 trials (53%);
- 52 trials in all;
- 9 Codex trials.

About 1 minute each (they drop it), but the cost to the app's users is real.

**Proposal.** Admit `autocomplete` with HTML's token list:
- web: the attribute;
- iOS: `textContentType` for the tokens UIKit has (`email`, `username`, `current-password`, `new-password`, `one-time-code`, `name`, `tel`, `postal-code`, …);
- macOS: `contentType`;
- tokens a platform lacks: nothing.

`aria-valuetext` came up a few times too and is a separate small ARIA admission.

**Cost.** A schema prop plus a mapping on each host. About half a lane-day, plus a native build and drive.

**Recommendation.** Do it.

### 3.7 `analyze-call-stale-read` through a derive — **leave the check; name the derive in its message**

**What builders hit.** `name = v` then `keep(Draft(current, name=v))` is refused. `current` is a derive that reads `name`, and the callee is reported to see the starting value. The copy overrides `name`, so this read is harmless. The fix is to move the call above the assignment or bind a `let`.

**How often.**
- 15 of 72 t7 trials (21%);
- 20 trials in all.

Median 2 minutes. Builders call the message "clear" and fix it at once.

**Proposal.** Making the analysis field-precise is one option: know which fields of `current` depend on `name`, and that the copy overrides exactly those. That is a real change to `contract/analyze`, with soundness risk, for a two-minute papercut.

A cheaper improvement: name the derive in the message ("`keep` reads `name` through `current` (line 8)"), so a builder sees why at once.

**Cost.**
- Field-precise analysis: several lane-days, plus a Lean story.
- The message: a few hours (it needs the derive name threaded through `slot_reads`).

**Recommendation.** Leave the check as it is. Name the derive when someone is next in `analyze/calls.rs`.

### 3.8 `storage.kv` — **an RFC: generalize the secrets snapshot to app settings**

**What builders hit.** One persisted setting (pomodoro minutes) means a JSON file through `storage.fs`: `mkdir`, `atomicWriteFile`, `TextEncoder`, a lazy first read. The `storage.kv` grant exists, but no data-module API does.

**How often.** 16 trials, 11 of them in t5 (16% of t5). About 5 minutes each.

**Proposal.** The runner's `Store` (`store.get`/`set` under `secret.keep`) already has the right shape for settings:
- a snapshot read before boot;
- synchronous reads;
- persisted writes.

It is scoped to secrets (Keychain, `localStorage`). Adding a non-secret scope (`app.keep <name>`, or `storage.kv`) would give settings a one-line API and a synchronous first frame. That second part also helps §3.9.

**Cost.** One to two lane-days across hosts, plus an RFC (where non-secret values live on each platform, size limits, and what a drive without `--storage` sees).

**Recommendation.** Worth an RFC. It is a common app need beyond the bench.

### 3.9 State that starts from a resource — **docs (the recipes exist); no language change**

**What builders hit.** "Show the saved value until the user edits it" cannot be `state x = saved.value`. Builders used a sentinel (`chosen = 0` with `derive length = chosen > 0 ? chosen : saved.minutes`), or the guide's child-component form.

**How often.** 10 trials explicitly, plus most of t5's settings work.

**Proposal.** LLP 1088 D4 settled the initializer's scope. The two patterns are documented:
- the derive-with-override;
- the child made once the record is in (whose native kept-answer trap is now a pitfall).

**Recommendation.** No change. If §3.8 lands, settings read synchronously at boot and much of this disappears.

### 3.10 `px` strings on `font-size` and `letter-spacing` — **admit them (the web is the standard)**

**What builders hit.** `padding="12px"` compiles, but `font-size="14px"` and `letter-spacing="0.5px"` are refused ("expected number; write `letter-spacing=0.5`").

**How often.** 13 Codex trials, about 6% of Codex runs. In practice it is nearly every Codex run that writes CSS from memory. 0 Claude trials. About 1 minute each.

**Proposal.** Take a `px` string wherever a number already means pixels, as `padding` does.

**Cost.** A lowering change and tests. Under half a lane-day.

**Recommendation.** Do it. It is an inconsistency, and CSS takes both.

### 3.11 `none` on `max-width`/`max-height` — **admit `none`; consider refusing `auto`**

**What builders hit.** `max-height="none"` (CSS's initial value) is refused, while `auto` (not a CSS maximum) passes. land47 made the refusal say "leave it out, or `auto`".

**How often.** 4 trials. The hint now costs under a minute.

**Proposal.**
- Admit `none` as the unbounded maximum.
- Under "ideal end states", refuse `auto` on a maximum, as CSS does.

**Cost.** Small: lowering and a schema value.

**Recommendation.** Do it, both halves.

### 3.12 Number-input `min`/`max`/`step` as strings — **take numbers too**

**What builders hit.** On `input type="number"`, `min=2` is refused (it wants `"2"`), while `type="range"` takes numbers. The refusal names the fix.

**How often.** 24 trials, mostly t7. About 1 minute each.

**Recommendation.** Take a number or a string on both. Small.

### 3.13 CSS `min()`, `max()`, `clamp()` — **an RFC; meanwhile a pitfall**

**What builders hit.** `padding-top="max(24px, env(safe-area-inset-top))"` is refused. This is the standard safe-area idiom. Builders put `env()` on an outer box and plain padding inside.

**How often.**
- 28 trials, 25 of them on iOS: 20% of iOS trials.
- Plus `clamp()` for `font-size` once.

About 1 minute each.

**Proposal.** Comparison functions over lengths need the kernel's length values to hold an expression. Today they hold `calc(<percent> ± <px>)` and `env()`. Resolution happens at layout, per host. That is a kernel and Taffy change with conformance work.

**Cost.** Several lane-days.

**Recommendation.**
- Now: a pitfall naming the two-box pattern.
- Later: an RFC for `min()`/`max()`/`clamp()` over the length types the kernel already has. It follows CSS and comes up often on iOS; it is just not cheap.

### 3.14 `position: fixed` — **leave it**

**What builders hit.** A toast or undo bar wants `position: fixed`. LLP 1001 declares it not a row. Builders used `sticky`, or an absolute overlay in a viewport-sized root (the refusal says how).

**How often.** 14 of 51 t6 trials (27%). About 1 minute each.

**Recommendation.** Leave it. The refusal's guidance works. Containing-block support (LLP 1074) could make it possible later, but nothing here needs it.

### 3.15 The default look of a bare `input` — **an RFC: give fields a platform look the way buttons have one**

**What builders hit.** A bare `input` on the web has no border; it looks like plain text. Builders found it only from a screenshot and styled it by hand. A bare `button` likewise draws as text.

This is deliberate: `host/web/index.html` resets `button, input, textarea` with `all: unset`, so a bare node is a bare box (LLP 1064 D6). Native buttons opt into chrome with `appearance="auto"` (LLP 1069.011).

**How often.**
- 11 trials;
- also builders' remarks that native buttons are "very small" in headless Chrome.

2–3 minutes each, found only by looking.

**The tension.** "The web is the standard" says an `<input>` has a visible field by default. Exact2's choice that a bare node is a bare box keeps every host's default identical and authored. Both are defensible.

**Proposal.** Mirror the button: `input appearance="auto"` draws the platform's field:
- web: the UA's field;
- iOS: a rounded-rect `UITextField`;
- macOS: `NSTextField`'s bezel.

Then decide, as a separate question, whether `auto` becomes the default for inputs.

**Cost.** One to two lane-days across hosts.

**Recommendation.** Worth an RFC. My lean is `appearance="auto"` for fields with the bare default kept, so nothing that exists changes. Make it the default only if you want inputs to follow CSS's default rather than exact2's bare-box rule.

### 3.16 A text field that does not snap back — **an RFC; the pitfall stands meanwhile**

**What builders hit.** A field bound to a value the action normalizes back to what it already held shows what was typed. Typing `-2` normalizes to the `0` already in state, and the field keeps `-2`. React writes the bound value back after every input; exact2 re-sets a field only when its binding changes.

The same family covers a checkbox bound to a resource field. It snaps back until the save answers (land41's pitfall), which is correct, and only surprising.

**How often.**
- 8 trials, mostly t1;
- 1 grader failure (codex17 t7).

About 5 minutes each.

**Proposal.** After an input's commit settles, write the bound value back to a text field whose content differs. The cost is in the corners:
- composition (IME) must not be interrupted;
- the caret must be kept;
- native text fields need the same rule (`UITextField`, `NSTextField`).

**Cost.** Two or more lane-days, with conformance and device work.

**Recommendation.** Worth an RFC if you want React's model. The pitfall is enough for authors meanwhile.

### 3.17 Driver silences — **do them (cheap, driver only)**

Each of these cost a builder minutes because nothing said what happened:

| Silence | Seen |
|---|---|
| A drive without `--storage`: storage writes are refused, said only in `logs` and a web CLI stderr note, not in an iOS reply or a JS drive script. land54 put `--storage` in `AGENTS.md`. | 10 |
| A reorder drag refused during the last drop's session: the drive's reply reads like a success. | 2 |
| A reorder grip whose touch the browser took (an ellipsis title is a scroll container): nothing is journaled. | 2 |
| A native first frame from a kept answer that the fresh answer contradicts: no development log line. | 5 |
| An iOS screenshot shortened by the keyboard: the reply does not say the keyboard is up. | 1 |

**Recommendation.** One driver lane: add a reply note or journal line for each. These are in `QUEUE.md` already.

### 3.18 Reorder: a second drag during a drop's session — **a D8 question**

**What builders hit.** LLP 1094 D8 refuses a new drag until the last drop's session ends: the hold until the move shows, then the landing (about 250 ms). A person who drags two cards quickly loses the second.

**How often.** It is what fails the bench's t4 requirement 2 (§4.1): 40 of 82 graded t4 runs.

**Proposal.** Finish the landing at once when a new drag starts, instead of refusing it. The first session completes and the second begins.

**Cost.** Host code in `reorder.js`, `group-glue.js`, the runner's `reorder_group.rs` and the native presenters. One to two lane-days.

**Recommendation.** Your call on D8. My lean is to admit the second drag. A refusal the user cannot see is worse than an abbreviated landing.

### 3.19 The JS target autofocuses only at boot — **fix (a parity bug)**

LLP 1035.000 D9 honours `autofocus` at mount, and the wasm target does. The JS target focuses the first `[autofocus]` once at boot. land44 documented a workaround: give the field an `id` and call `focus(id)` in the action.

**Recommendation.** Fix it. It is a bug against an accepted decision. Small.

### 3.20 iOS date inputs — **investigate**

**What builders hit.** An empty date input draws today's date (`UIDatePicker` has no empty state; land41's pitfall). On iOS, typing a date past `max` reportedly left the value empty while the web took it. `typeDate` dispatches the typed text, so this is unexplained. Two trials reported a date input drawn offset in a flex row.

**How often.** 11 trials, 9 of them on iOS.

**Recommendation.** One iOS lane:
- reproduce the `max` case;
- draw an empty state as macOS's `DateField` does;
- check the control's sizing.

### 3.21 Smaller language asks — **no action now**

Each of these came up one to three times:
- a `??` operator for options;
- top-level constants;
- an `isSome`-style test;
- `let` inside a view;
- a per-node conditional class beyond `class=(c ? A : B)`, which exists;
- an `else empty(…)` clause on a continuation line (13 trials; the refusal names the fix, about 1 minute);
- a `progress` tag or spinner, and an `icon` tag;
- `expect text != …` and `contains` in tests.

**Recommendation.** No action now. The continuation line is the only one I would take soon: it is small, and long `empty(…)` lines are a real annoyance.

### 3.22 Test-file gaps — **two cheap steps now**

| Gap | Seen | Recommendation |
|---|---|---|
| No browser-back step in a test file (only the CLI's `tap <root> history -1`) | 8 | Add `back` (web; native refuses as the CLI does). Small. |
| No per-host test (iOS reload reopens at launch, so builders split web-only files) | 7 | Add `test "…" on web`. Small. |
| No assertion on an attribute (`aria-pressed`) | 3 | Wait for `tree` to carry the states (`QUEUE.md`'s `--ax pressed`). |
| No way to preload a store for a migration test | 4 | Later; §3.8 changes what a store is. |

## 4. The bench itself

### 4.1 Two grader fixes (they need your yes: graders change only with a human's say)

- **t4 requirement 2** drags two cards in a row, waiting 250 ms after each release. Exact2 holds a drop until its move shows, then lands it, and refuses a drag until then (§3.18). The second drag fails in 40 of 82 graded runs, whatever the author does. Fix: wait until the board is settled (no lifted card, the moved card in its column) before the second drag, with a cap.
- **Clicking a disabled button** ("blank adds nothing") times out as a driver failure, though nothing was added. Fix: treat a disabled control as the requirement met when nothing changed.

### 4.2 New tasks (they need your yes)

The seven tasks are close to saturated: the last rounds pass at 100%. The next authoring problems live in what the tasks do not exercise:
- multi-screen navigation with deep links;
- gestures and animation;
- an app that syncs or works offline;
- forms with heavier validation;
- a canvas or game surface;
- a native module.

Two or three new tasks would find more than more rounds of these.

### 4.3 Android

Never run. Exact2 has no Android host verb, and the Hetzner boxes have no `/dev/kvm`. It needs infrastructure first.

### 4.4 Cadence

One regression round a day (all three machines) catches problems as main moves, at about a third of the current cost.

## 5. Open questions for Charlie

1. Accept §3.1–§3.2 (`parseNumber`, `ceil`, `round`, `"fixed"` with `toFixed`'s rounding)?
2. Accept §3.3's source fault injection (fail and hold, in the runner) as a small RFC?
3. §3.15: should a bare input stay a bare box, with `appearance="auto"` as an opt-in field? Or should fields default to the platform's look, as CSS does?
4. §3.18: admit a second drag during a drop's session (amending LLP 1094 D8)?
5. §3.16: do you want React's write-back for text fields?
6. §4.1: may the two graders change? §4.2: may new tasks be added, and which?
7. §4.4: cut to one round a day?

## 6. Revisions

- r1, 2026-10-06: first draft, from 659 counted trials (r1–r41, codex2–codex31, ios2–ios33; r24 and the noise runs excluded).
