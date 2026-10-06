# LLP 1102: What the authoring bench asks for — the decisions left after the loop

**Type:** RFC (a decision brief: each item proposes, Charlie decides)
**Status:** Decided in part (Charlie, 2026-10-06; §0). r2 folded in one blind pass by Astra (`gpt-6-astra`, xhigh) and Grok 4.7 (xhigh); no further rounds.
**Systems:** Contract (`contract/{syntax,types,analyze,lower}`), the roster (`plan/tables/format.json` `stdlib`), the runner, the JS target (`host/web-js`), the web host (`host/web/index.html`'s control reset), the Apple hosts, the kernel's length values, the agent driver (`scripts/agent*.mjs`) and the authored-test grammar, the data module's `storage`, the Lean semantics and difftest (for any roster change), and the authoring bench itself (`ccheever/authoring-bench`: graders, tasks)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Implementer:** none yet. Each item that Charlie accepts gets its own lane (and, where marked, its own RFC).
**Related:** LLP 1087 (the authoring bench; this is its §8.1 step 4, "what needs a human"); LLP 1088 (what the app diaries ask of Contract: D2 deferred numeric parsing with a trigger this bench has now met, §3.1); LLP 1054.000.003 (the formatters); LLP 1092 (gated tasks); LLP 1094 D8 (a drag during the last drop's session); LLP 1035.000 D9 (`autocomplete` at mount); LLP 1001 (`position: fixed` is not a row); LLP 1064 D6 and LLP 1069.001 (the web's control reset); the bench's findings registry, `analysis/findings.md` in `ccheever/authoring-bench`

## 0. Charlie's decisions (2026-10-06)

| § | Question | Decision |
|---|---|---|
| §3.1, §3.2, §3.4, §3.6, §3.10–§3.12 | `parseNumber`, `ceil`, `round` (`Math.round`), the `"iso"` date style, `autocomplete`, `px` strings, `none` on `max-*`, number-input bounds as numbers | **Accepted.** Build them. §3.1 `parseNumber`, §3.2 `ceil` and `round`, §3.4 `"iso"`: **built e6e9ea5e7** (review fixes aad86ff08, c768b5442). §3.10 `px` strings (pixel rows and SVG's stroke lengths), §3.11 `none` (kept beside `auto`; every writer prints `none`), §3.12 number-field bounds: **built 170291022** (review fixes f19e25768, 62a328bb7). §3.6 `autocomplete`: **built bff8c035d** (review fixes f1db65327, de87adf03): HTML's grammar; `off` clears; `on`, a refused list, or a name with no Apple type keeps the type-derived content type. |
| §3.2 | Money: `toFixed` or a cents-based function | **Decided (c), built d67ba9c20** (review fixes 2fbc035cc, 31197237a): both. `toFixed(n, digits)` is JavaScript's, binary rounding included, with one declared difference: a non-finite `n` prints `""` (LLP 1054.000.003 D7). `formatDecimal(units, digits)` prints an integer count of a smallest unit exactly, and `""` for a non-integer. Digits are whole-number literals (0–100, 0–20). The guide's money recipe is a count of cents, or `round(price * 100)` for a price of at most two decimals under a trillion. |
| §3.4 | `calendarDiff` | **Accepted, narrowly:** whole years and months between two ISO dates, `option<number>`. "Nice to have; let's try adding it for now." **Built e6e9ea5e7**: counted as Temporal's `PlainDate.until` counts (a Feb 29 start completes a year on Mar 1 of a common year, a Jan 31 start a month on Mar 1). |
| §3.3 | Source faults (`fail`, `hold`) as a failed host request | **Accepted** as recommended: a small RFC, resources first, then build. |
| §3.15 | Text fields: visible by default, or opt in | **Visible by default.** A field draws the platform's field; `appearance="none"` keeps the bare box. Buttons keep their rule. **Built e97afa5af** (LLP 1104: default rows, not native chrome; focus states deferred). |
| §3.16 | The editing contract instead of write-back | **Accepted.** Docs and a recipe; no automatic write-back. |
| §3.18 | Reorder | **Keep D8's hold; cut the landing short** when a new drag starts. |
| §4.1, §4.2, §4.4 | Graders, new tasks and coverage, cadence | **Accepted.** Fix the two graders, add two or three tasks and the coverage checks, then one round a day. |

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
| `parseNumber` with a stated grammar (§3.1); `ceil` and a `round` that is `Math.round` (§3.2); the `"iso"` date style (§3.4); `px` strings, and `none` beside `auto` (§3.10, §3.11); number-input bounds as numbers (§3.12); `autocomplete`, a documented subset (§3.6); the JS target's autofocus at mount (§3.19); the driver's silences (§3.17); the two grader fixes (§4.1) | a source fault as a failed host request (§3.3); a fixed-decimal or money function (§3.2); `calendarDiff` (§3.4); the default look of a bare input (§3.15); an editing contract for fields (§3.16); `max(length, env())` (§3.13); `storage.kv` (§3.8) | a complete fetch-timeout recipe, demonstrated (§3.5); state that starts from a resource (§3.9) | `position: fixed` (§3.14); the stale-read check's strictness (§3.7); D8's hold (§3.18) |

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

### 3.1 Parsing text to a number — **do it, with the grammar stated**

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

**LLP 1088 deferred this** with a trigger: "a view that needs a number from text on each keystroke, with no domain meaning". The bench meets that trigger in nearly two thirds of the trials that can. 1088's concern was footguns: `Number("")` is 0, and `NaN` spreads. A parse that returns an option answers both, provided the grammar closes the holes below.

**Proposal.** `parseNumber(text): option<number>`. Grammar:

- Leading and trailing whitespace are `trim`'s set (JavaScript's, `is_js_space`), not Rust's.
- An optional sign.
- Then either digits with an optional fraction, or a fraction alone (`".5"` is 0.5).
- `"5."` is 5, as JavaScript's `Number` reads it.
- An optional decimal exponent (`1e3`), so that `parseNumber(toString(n))` returns `n` for every finite `n`. `toString` prints exponents at large and small magnitudes.
- Anything else is `none`: `""`, hex, `Infinity`, `NaN`, trailing junk (`"12px"`, so the `parseInt` idiom must not promise otherwise).
- A finite result is required: a digit string that overflows is `none`, not `Infinity`.
- A nonzero digit string that underflows to 0 is also `none`, or the footgun returns as a silent 0.
- `"-0"` parses to −0, which Lean already distinguishes.

The parse should be the existing correctly rounded one in `num/src/lib.rs`, so the runner, the JS target (where `Number()` is correctly rounded) and Lean agree.

`parseInt` and `Number` keep their `idioms.rs` refusals, which then name `parseNumber` and say what it rejects.

**Cost.** One roster function: plan table, types (`option<number>` is a new arm in `from_roster`), runner, JS target with its budget, Lean, difftest, docs. About one lane-day; both reviewers found that plausible.

**Recommendation.** Do it.

**A footnote for §3.16.** A field that writes `parseNumber`'s result back to itself makes `"-"` and `"1."` impossible to type, because both parse as `none` (or lose the dot) while the user is mid-edit. Keep the raw text in state and parse it where the number is used.

### 3.2 `ceil`, `round`, and money — **`ceil` and `round` now; a decimal function needs its own decision**

**What builders hit.** Showing `$12.34` needs two decimals. Rounding up a per-person share needs `ceil`. The roster has `floor`, `min` and `max` only. Builders wrote `0 - floor(0 - x)` for `ceil`, or moved money math into a source.

**How often.** 55 trials:
- t1: 41 of 65 (63%);
- t5: 10 of 70.

Median 2 minutes; the bigger cost is the same source round trip as §3.1.

**`ceil` and `round`.** `ceil` is an hour across the stack.

`round` is `Math.round`, and not Rust's `f64::round`:
- `Math.round` breaks ties toward +∞, so `Math.round(-1.5)` is −1 where `f64::round` gives −2;
- `Math.round(-0.5)` is −0, which Lean keeps.

The runner and Lean are the work; the JS side is `Math.round`.

**A fixed-decimal format is not one obvious function.** r1 proposed `formatNumber(n, "fixed", digits)` following `toFixed`. The reviews found three problems:

- **It breaks an accepted rule.** LLP 1054.000.003 D7 prints `""` for a non-finite value in every format entry, on purpose; `toFixed` prints `"NaN"` and `"Infinity"`.
- **It breaks the roster's shape.** `formatNumber`'s styles are a closed literal set at one arity, and `formatNumber` follows Intl.
- **`toFixed` has more edges than ties:** negative zero, non-finite values, exponential output at magnitudes of 10²¹ and more. On the tip path the binary value is the trap: `(1.005).toFixed(2)` is `"1.00"`, because 1.005 is stored just below the midpoint.

Two designs are worth choosing between:

1. **`toFixed(n, digits)` under its familiar name,** with the web's rounding, and D7's non-finite rule stated as a written exception or applied to it. Agents know the name. Its binary-value rounding surprises people exactly as it does on the web.
2. **A money-shaped function that rounds to integer units and pads,** for example `formatDecimal(round(x * 100), 2)` over an integer count of cents. Arithmetic in cents avoids the 1.005 trap.

`num/src/text.rs` has an exact fixed formatter either way. Both need tests in lockstep across `runner/src/format.rs`, `host/web-js/format.js` and `Contract/Format.lean`. That is about a lane-day, not half.

A currency style (`formatNumber(n, "currency", "USD")`) belongs with the existing `format` capability, which is locale-dependent. It should wait for a consumer who needs more than `$` and two decimals.

The `idioms.rs` hint for rounding should change too: it cannot preserve trailing zeros.

**Recommendation.**
- `ceil` and `round`: now.
- The decimal format: pick (1) or (2). I lean to (2) for money and (1) only if you want the web's name with its traps.

### 3.3 Making a source fail (or hang) in a test or drive — **do it, as a failed host request; an RFC**

**What builders hit.** The recipe task asks for an error state and a retry. No authored test can make a fetch fail.

What builders did instead:
- read `scripts/agent.mjs` to find raw CDP (`s.carrier.call('Network.setBlockedURLs')`);
- wrote a fault proxy and pointed a copy of the app at it (a copy outside its folder also needs `exact.mjs update`);
- on iOS, rebuilt against a dead port.

**How often.** 44 of 69 t3 trials (64%), plus 7 Codex trials. Median 5 minutes; 207 minutes reported in 26 reports. ios24 t3 alone reported about 30 minutes.

**What to inject: the right failure.** r1 proposed answering the source "as a failure". Both reviews pointed out that this means two different things in the runner, and r1 picked the wrong one.

- **A host request that fails** (a fetch's network error) is an `Outcome::Failed` that the source's own code sees. A source that catches it and returns an error record (what the recipe apps wrote) turns it into a value (`fulfill_inner` in `commit.rs`).
- **`failed(resource)`** is set only when the source's parse throws (`release_failed` in `admission.rs`). The view keeps the previous value. `failed()` is a bool, with no message.

Writing the flag directly would skip the error handling the apps actually have. What the builders staged with CDP is the first: the host request fails, and the source still runs.

**Proposal.** A driver op and a test step that act on the next host request a source makes, not on the resource's flag:

```text
fail "recipes"        # the source's next host request (fetch, storage) fails; the source runs
hold "recipes"        # that request stays in flight until released
release <ticket>      # by the ticket hold reported, not by name
```

- Intercept at the host outcome, before any side effect.
- Release by ticket, not by source name: a source can have several requests in flight.
- A held request must be counted the way device holds are (`holds()` in `host/web-js/agent.js`). Otherwise `inflight` makes every later drive step wait out its 20-second budget. The device holds supply that settlement plumbing; the rest is new.
- The first version covers resource requests only. Mutations (with `queue`) and streams need their own rules.

`hold` also makes a timeout testable (§3.5). It is exactly the case ios21 t3 could not stage.

**Cost.** The outcome hook in the runner and in `rt.js`, the op on each carrier, the test grammar and runner, docs. Two to three lane-days is a floor for resources alone; mutations and streams add more.

**Recommendation.** Do it, as a small RFC that states the injection point, the ticket model and what a queued mutation does. It would end the bench's single largest time sink.

### 3.4 ISO dates and the age check — **`"iso"` now; `calendarDiff` is the real ask**

**What builders hit.** "At least 13 years old" needs today's date as `YYYY-MM-DD` to compare with a date input's value, and then "13 years before today".

Builders did one of two things:
- passed `exactTime().epochAtZero + now()` into a source that formats it;
- wrote civil-from-days arithmetic as about fifteen `fn`s.

**How often.** 27 of 72 t7 trials (38%). Median 10 minutes, the highest per-occurrence cost in this list.

**Two parts.**

- **`formatDate(ms, offsetMinutes, "iso")`, giving `YYYY-MM-DD`.** `civil_from_days` already runs in `runner/src/format.rs` and in `x_formatDate`, so this is another return in two functions plus Lean. It is an hour or two, not half a day, and needs no RFC (r1's table misfiled it).
- **The 10 minutes are the age check,** which an ISO string does not compute. LLP 1054 held `calendarDiff` (returning `option<number>`, never `NaN`) for "a view branches on a calendar day". t7 is that caller.

Comparing two ISO strings already works (1088 D1).

**Recommendation.**
- `"iso"`: now.
- `calendarDiff`: either specify it, or record that its trigger is met and leave it in the data module on purpose.

### 3.5 A timeout on a fetch — **docs, but a complete and demonstrated recipe**

**What builders hit.** A server that hangs shows "Loading…" forever. A data source has no timers, so there is no `setTimeout` and no `AbortSignal.timeout`.

**How often.** 32 of 69 t3 trials (46%). Median 3 minutes. Most builders noted it and moved on.

**r1's recipe was incomplete.** A gated task (`task waiter when pending(recipes)` with `after(10000, giveUp)`) is a loading deadline, not a fetch timeout:

- `giveUp` does not cancel the request, clear `pending`, or stop a late answer from landing.
- A retry while `pending` stays true leaves the timer spent: LLP 1092 re-arms a gated task only when its key changes.

**Proposal.** Write the recipe whole:
- an attempt counter as the task's key, so each retry re-arms the deadline;
- what the view shows after the deadline;
- what a late answer does (it lands, and is shown or ignored by the attempt it belongs to).

Demonstrate it running, with §3.3's `hold`, before calling the case solved. A host-side `fetch(url, { exactTimeout })` would need an agent-clock story and duplicate the Contract form.

**Cost.** Docs and a fixture: half a day once §3.3's `hold` exists.

**Recommendation.** The recipe, demonstrated. At 46% of t3 trials it is worth getting exactly right.

### 3.6 `autocomplete` on inputs — **do it, as a documented subset**

**What builders hit.** HTML's `autocomplete="email"` and friends are refused (`lower-unknown-attr`). A sign-up form loses autofill and password-manager hints.

**How often.**
- 38 of 72 t7 trials (53%);
- 52 trials in all;
- 9 Codex trials.

About 1 minute each (they drop it), but the cost to the app's users is real.

**Proposal.** Admit `autocomplete` as a string, with a documented subset and two rules the reviews found:

- **HTML's value is an ordered token list** (`"section-a shipping street-address"`), not one word. Admit the string. The host maps its last field token, and ignores the section and contact tokens a platform lacks.
- **It overrides the type-derived content type.** `NodeViewIOS` already sets `textContentType` from `type` (password becomes `.password`, email `.emailAddress`). A sign-up's `new-password` must win over that.
- **`off` clears it.** Dropping `off` would leave the password type in place, so the web would suppress autofill while iOS would not.

Mapping:
- web: the attribute;
- iOS: `textContentType` for the tokens UIKit has (`email`, `username`, `current-password`, `new-password`, `one-time-code`, `name`, `tel`, `postal-code`, …);
- macOS: `contentType`.

`aria-valuetext` came up a few times too and is a separate small ARIA admission.

**Cost.** About half a lane-day for the string and that map, plus a native build and drive. Verifying real autofill on a device is more.

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

### 3.8 `storage.kv` — **an RFC for a settings API; it does not fix the first frame by itself**

**What builders hit.** One persisted setting (pomodoro minutes) means a JSON file through `storage.fs`: `mkdir`, `atomicWriteFile`, `TextEncoder`, a lazy first read. The `storage.kv` grant exists, but no data-module API does.

**How often.** 16 trials, 11 of them in t5 (16% of t5). About 5 minutes each.

**Proposal.** The runner's `Store` (`store.get`/`set` under `secret.keep`) is already a synchronous snapshot read before boot, with persisted writes. It is scoped to secrets. A non-secret scope (`app.keep <name>`, or `storage.kv`) would give settings a one-line API.

r1 claimed that this would also make settings available in the first frame and remove §3.9's workaround. It would not by itself. Data-module activation and resource initialization are separate from the store. A store read does not make `state x = saved.value` legal, and it does not deliver a fresh setting before first paint. Those are separate promises, and only the API is cheap.

**Cost.**
- The API: one to two lane-days across hosts, plus an RFC (where non-secret values live on each platform, size limits, what a drive without `--storage` sees).
- The first-frame promise: a further design.

**Recommendation.** An RFC for the API. Keep the first-frame question out of it.

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

**Proposal.** `font-size="1.5rem"` already resolves (`style/relative.rs`). `"14px"` fails only because those `f32` rows never call `parse_pixel_length`. The fix is that kernel arm, keeping each row's sign rule: `font-size` refuses a negative, `letter-spacing` allows one.

**Cost.** Well under half a lane-day.

**Recommendation.** Do it. It is an inconsistency, and CSS takes both.

### 3.11 `none` on `max-width`/`max-height` — **admit `none` as an alias of `auto`**

**What builders hit.** `max-height="none"` (CSS's initial value) is refused, while `auto` passes. land47 made the refusal say "leave it out, or `auto`".

**How often.** 4 trials. The hint now costs under a minute.

**Proposal.** Admit `none` as the same value `auto` lowers to: Taffy's unbounded maximum (`Size::auto()`).

r1 also suggested refusing `auto` on a maximum, as CSS does. Grok pointed out that this would break land47's hint and every app that followed it, for four trials. Keep `auto`.

**Cost.** Small: lowering and a schema value.

**Recommendation.** Admit `none`; keep `auto`.

### 3.12 Number-input `min`/`max`/`step` as strings — **take numbers too**

**What builders hit.** On `input type="number"`, `min=2` is refused (it wants `"2"`), while `type="range"` takes numbers. The refusal names the fix.

**How often.** 24 trials, mostly t7. About 1 minute each.

**Proposal.** `controls.rs`'s `range_attrs` already rewrites `min`, `max`, `step` and `value` for `type="range"`. Extend it to `type="number"`.

**Recommendation.** Do it. Small.

### 3.13 CSS `min()`, `max()`, `clamp()` — **the safe-area case first; the general form is an RFC**

**What builders hit.** `padding-top="max(24px, env(safe-area-inset-top))"` is refused. This is the standard safe-area idiom. Builders put `env()` on an outer box and plain padding inside.

**How often.**
- 28 trials, 25 of them on iOS: 20% of iOS trials.
- Almost all are that one idiom; `clamp()` for `font-size` came up once.

About 1 minute each.

**Proposal.** `env()` is already a `Dimension` the kernel resolves before Taffy. So `max(<length>, env(…))` (and `min()`) can be one more kernel kind, resolved the same way, without teaching Taffy a grammar for comparison functions. That covers the case the bench hit.

The general form (any mix of percentages, viewport units and `calc()` inside `min()`/`max()`/`clamp()`) needs the kernel's length values to hold an expression, with conformance work. It is several days.

**Cost.**
- The `max(length, env())` kind: about a lane-day with conformance.
- The general form: an RFC.

**Recommendation.** A pitfall naming the two-box pattern now. The `max`/`min`-with-`env()` kind next, since it is what iOS authors write.

### 3.14 `position: fixed` — **leave it**

**What builders hit.** A toast or undo bar wants `position: fixed`. LLP 1001 declares it not a row. Builders used `sticky`, or an absolute overlay in a viewport-sized root (the refusal says how).

**How often.** 14 of 51 t6 trials (27%). About 1 minute each.

**Recommendation.** Leave it. The refusal's guidance works. Containing-block support (LLP 1074) could make it possible later, but nothing here needs it.

### 3.15 The default look of a bare `input` — **an RFC: give fields a platform look**

**What builders hit.** A bare `input` on the web has no border; it looks like plain text. Builders found it only from a screenshot and styled it by hand.

This is deliberate. `host/web/index.html` resets `button, input, textarea` with `all: unset`, so a bare node is a bare box (LLP 1064 D6). The native hosts agree: iOS sets `borderStyle = .none`, and macOS sets `isBezeled = false`. Native buttons opt into chrome with `appearance="auto"` (LLP 1069.011).

**How often.** 11 trials. 2–3 minutes each, found only by looking.

Builders also called native buttons "very small" in headless Chrome. That is a separate bug, and should not steer this decision.

**The tension.** "The web is the standard" says an `<input>` has a visible field by default. Exact2's choice that a bare node is a bare box keeps every host's default identical and authored.

The reviewers split:
- Astra leans to visible fields by default: the repository has no compatibility obligation, so keeping existing apps unchanged is weak reason.
- Grok notes that opt-in matches the button and the bare field is the current cross-host default.

**What it takes either way.**
- **The web.** A field look on the web is the button's revert list (background, border, padding, font, colour, which `all: unset` removed), not just `appearance: auto`. Checkboxes, radios and date inputs already force `appearance: auto` in that sheet; one rule for text fields must not fight them.
- **Measurement.** The field's chrome has to enter measurement on every host. `TextInput` measures by a different path from `Control` (`kernel/src/layout.rs`).
- **Native.** iOS's rounded-rect field and macOS's bezel.

**Cost.** One to two lane-days is a lower bound.

**Recommendation.** An RFC. Decide first whether fields default to visible (CSS's default) or opt in with `appearance="auto"` (the button's rule). I now lean to visible by default for text fields: it is what every builder expected, and the bare look is rarely wanted on a field.

### 3.16 A text field that does not snap back — **an editing contract first, not automatic write-back**

**What builders hit.** A field bound to a value the action normalizes back to what it already held shows what was typed. Typing `-2` normalizes to the `0` already in state, and the field keeps `-2`. React writes the bound value back after every input; exact2 re-sets a field only when its binding changes.

The same family covers a checkbox bound to a resource field. It snaps back until the save answers (land41's pitfall), which is correct, and only surprising.

**How often.**
- 8 trials, mostly t1;
- 1 grader failure (codex17 t7).

About 5 minutes each.

**Why not just write back.** Both reviews argued against React's rule as the default:

- **It breaks intermediate input.** Writing back after every settled input makes intermediate text impossible: with §3.1's `parseNumber`, `"-"` and `"1."` parse as `none` (or lose the dot) while state still holds the old number, so they are erased as they are typed.
- **It erases drafts.** Fields whose draft is saved asynchronously, or on blur or Enter, would lose it. `apps/markdown` binds `value=… change=…`.
- **The widget work already exists.** The write path already defers composition and carries the caret (`glue.js` `writeValue`, `FieldEditingIOS`). The missing part is the policy, not the code.

**Proposal.** State an editing contract and make the guide's form its recipe:
- raw text in state while editing;
- validation against the parsed value;
- normalization on commit (`change`, Enter, blur), which changes the bound value and redraws the field.

Then consider a narrow reconciliation only for a synchronous `input` handler that writes back the value it was given.

**Cost.** The contract and recipe: docs. Any reconciliation: an RFC, smaller than r1 estimated once the policy is narrow.

**Recommendation.** The contract and recipe now. Leave automatic write-back out.

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

### 3.18 Reorder: a second drag during a drop's session — **keep D8's hold; at most cut the landing short**

**What builders hit.** LLP 1094 D8 refuses a new drag until the last drop's session ends: the hold until the move shows, then the landing (about 250 ms). A person who drags two cards quickly loses the second.

**The 40 of 82 is the grader, not D8.** `host/web/group-glue.js` refuses a lift while a session is current, and the landing spring is 250 ms. The grader waits 250 ms after release. With `clock settle` between drags, both cards move. Fix the grader (§4.1).

**Proposal.** D8 separates holding from settling:

- **Holding** means the move is still outstanding. Ending it early when a second drag starts would drop the pin against a board that is about to refresh. The next drag could see stale positions.
- **The landing** comes after the move has shown. Cutting the spring short when a new drag starts is a small product choice and safe.

**Recommendation.**
- Keep the hold.
- Optionally cut the landing short when a new drag starts.
- Keep a separate rapid-second-drag check in the bench (§4.2), so the grader fix does not hide the usability question.

### 3.19 The JS target autofocuses only at boot — **fix (a parity bug)**

LLP 1035.000 D9 honours `autofocus` at mount, and the wasm target does (`navigation.js` scans on mount). The JS target focuses the first `[autofocus]` once at boot (`rt.js`). land44 documented a workaround: give the field an `id` and call `focus(id)` in the action.

`checkpoint.js` already restores focus across a carried restart, so the fix is the mount scan.

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

### 4.2 New tasks and missing coverage (they need your yes)

The seven tasks are close to saturated: the last rounds pass at 100%. Passing them does not establish the behaviours this document is about.

Coverage the current tasks lack:
- a late answer, and a retry after a deadline (§3.3, §3.5);
- draft editing with commit-time normalization (§3.16);
- a submit button that enables only when the form is valid (the disabled-button grader case);
- two quick drags (§3.18).

The next authoring problems also live in what the tasks do not exercise:
- multi-screen navigation with deep links;
- gestures and animation;
- an app that syncs or works offline;
- a canvas or game surface;
- a native module.

Two or three new tasks, and checks for the coverage above, would find more than more rounds of these.

### 4.3 Android

Never run. Exact2 has no Android host verb, and the Hetzner boxes have no `/dev/kvm`. It needs infrastructure first.

### 4.4 Cadence

One regression round a day (all three machines) catches problems as main moves, at about a third of the current cost. Add §4.2's coverage before relying on it.

## 5. Open questions for Charlie

1. Accept §3.1 (`parseNumber`, grammar as stated), §3.2's `ceil` and `round` (as `Math.round`), §3.4's `"iso"`, and the small CSS and attribute items (§3.6, §3.10–§3.12)?
2. §3.2: for money, `toFixed` under the web's name with its rounding (and a stated D7 exception), or a cents-based decimal function?
3. §3.4: specify `calendarDiff` now that t7 meets LLP 1054's trigger, or keep date arithmetic in the data module?
4. §3.3: accept source faults as a failed host request (RFC), with resources first?
5. §3.15: should text fields default to a visible platform field (CSS's default), or opt in with `appearance="auto"` (the button's rule)?
6. §3.16: accept the editing contract (raw text while editing, normalize on commit) instead of React's write-back?
7. §3.18: keep D8's hold; cut the landing short on a new drag?
8. §4.1–§4.2: may the two graders change, and may new tasks and coverage checks be added? §4.4: one round a day?

## 6. Revisions

- r1, 2026-10-06: first draft, from 659 counted trials (r1–r41, codex2–codex31, ios2–ios33; r24 and the noise runs excluded).
- r3, 2026-10-06: §0 records Charlie's decisions.
- r2, 2026-10-06: one blind pass each by Astra (`gpt-6-astra`, xhigh) and Grok 4.7 (xhigh), folded in. No further rounds, by Charlie's direction. Changed:
  - §3.1's grammar is stated (overflow, underflow, `.5`, `5.`, `-0`, exponents, `trim`'s whitespace, trailing junk), with the `num` crate's parser.
  - §3.2 no longer proposes `"fixed"` on `formatNumber` (D7, closed arity, the binary-value trap); `round` is `Math.round`.
  - §3.3 injects a failed host request, not `failed(resource)`; release by ticket; resources first.
  - §3.4 separates `"iso"` (trivial) from `calendarDiff` (the real ask).
  - §3.5's recipe needs an attempt key and a late-answer policy, demonstrated.
  - §3.6 handles token lists, the type-derived content type and `off`.
  - §3.8 separates the settings API from a first-frame promise.
  - §3.10–§3.13 cost corrections.
  - §3.11 keeps `auto`.
  - §3.15 names the revert list and the measurement path, and records the reviewers' split.
  - §3.16 proposes an editing contract instead of write-back.
  - §3.18 keeps D8's hold.
  - §4.2 adds coverage the tasks lack.

