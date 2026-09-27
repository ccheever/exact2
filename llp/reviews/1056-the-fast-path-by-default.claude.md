# Panel: LLP 1056 with 1054.000.003 r2, .004 (withdrawn), .005, .006, 2026-09-26 (claude)

- **Family:** Anthropic, Claude Opus 5.5, as a Claude Code subagent. There is no effort setting on that path; the brief asked for maximum effort. Same family as the author.
- **Brief:** the shared panel brief (see the grok artifact's header), read-only at `5a303070`; probes under `/tmp/rv56`.
- **Redacted:** no. The final answer follows verbatim.

---

# Panel review: LLP 1056 and 1054.000.003 r2, .004 (withdrawn), .005 and .006

**Reviewer:** Claude (Opus 5.5), independent. Read-only at `5a303070`. My probes are under `/tmp/rv56`, built with `target/debug/contract`, and the Intl checks ran on Node 26.9.0.

---

## LLP 1056: The fast path by default

### Assessment
The thesis holds up against the code: the runner is sound, but the defaults are slow. P2 and P5 are clearly worth doing. P1 is worth doing only in a smaller form, and its linking mechanics are wrong as written. P3 and P4 contradict each other. P6's static report is both unsound and noisy on in-repo apps, and P6's counters mostly repeat information the runner already records in `logs`.

### Concerns

**§1, the analysis**
- **MATERIAL: important in-repo cases are missing.**
  - RealWorld, which §1 names as the natural template, polls every 100 ms because nothing runs when a mutation's reply lands. `action settle` (`apps/realworld/app.contract:197-219`) runs forever. This is the clearest "slow way by default" in the repo, and the analysis doesn't name it.
  - Messages passes `now()` straight into a resource's arguments: `deletedInbox = recentlyDeleted(…, now(), …)` (`apps/messages/app.contract:101`). Its source computes a days-left label from it (`apps/messages/app.ts:388-400`). By my reading, it is re-asked on every commit, including the 300 ms `tick` (`:465-470`). That is a worse and more relevant case than Caltrain.
  - View state is passed as resource arguments. `contacts` takes `newDraft` (`:102`), so every keystroke re-asks it.
- **MINOR: "eight list resources".** `session` is not a list (`~/projects/bluesky-exact2/app.contract:49`). The count is seven lists plus `session`.
- **MINOR: "RealWorld… uses whole lists" (R4).** Its feed is already a 10-row page per answer (`apps/realworld/app.ts:86-99`). Only comments and tags come back whole.
- **MINOR: "A plan links only the capabilities it uses."** That is true on the web only. Native hosts and tests boot with `RunnerLinks::ALL` (`runner/src/runner.rs:329-331`).
- **MINOR: the diary quote is misapplied.** R3 cites `DIARY.md:46-52` under Rust's friction. The quote is actually about TypeScript on Apple (Hermes isn't provisioned), which is why the port chose Rust. That weakens "TypeScript is the paved path".
- **Checked and accurate:** Post has 38 fields (`shapes.contract:74-112`); `Step` has 24 variants; `mutation change` declares 9 refreshes (`app.contract:58`); RealWorld's `change` serves 6 writes and refreshes 4 (`:120,158-185`); `settlement.rs:382-405` and `:557-563` say what §1 says; only messages-stress uses edge events.

**P3 and P4 contradict each other**
- **MATERIAL: P4's feed design is impossible as stated, and it would make P3 unnecessary.**
  - "One resource per mounted list" can't be declared for Bluesky's pinned feeds. Their number comes from data (`each f in feeds`, `screens.contract:240-242`), resources live in the root, and there is no per-row resource (`type-child-resource`).
  - Where one resource per list *is* possible, a zero-argument edge action can read its own resource's cursor. messages-stress already does exactly that: `reachEarlier writes cursor / cursor = history.earlier` (`apps/messages-stress/app.contract:75-80`). No P3 is needed.
  - P3 is needed only if the `list<Feed>` design stays. Even then, Contract has no maps and no list indexing, so it can't keep a window per feed in state. The windows have to live in the source, moved by a mutation plus a declared refresh.
  - P3 and P4 need to be designed together, as one feed design.
- **MATERIAL: P4 is ordered before its own evidence.** Bluesky is a Rust source, and §1 admits its cost is "mostly invisible". After P1 and P5, what bounded feeds would still save is unmeasured. P4 is also the most invasive change: anchoring, backward windows, and a source that keeps every page (LLP 1027.004 D2). Gate it on §5's residual numbers.

**P5: refreshes by kind of write**
- **MINOR: the §4 success test is wrong.** "A favorite on a feed page asks `feed` only" fails, because `favorite` must also refresh `post` for the article page's button. Off the article page, `post` is still asked; it just answers empty at once (`app.ts:106-107`). Write the test as "asks `feed` and `post`; one GET".
- **MINOR: the cost of splitting is understated.**
  - Each new mutation needs its own stamp, ok and error derives, plus a branch in `settle`. RealWorld's redirect logic and `EditorPage`'s `change` prop depend on the single `change` (`app.contract:124-127, 212-217, 272, 759-792`).
  - "No way to go stale" is overstated. A split mutation that leaves out a resource goes stale the same way. It is simply easier to audit.

**P6: making an expensive pattern visible**
- **MATERIAL: the static "re-asked by time" line is incomplete, and it is noisy on the admitted apps.**
  - It misses `refresh r` inside a timer's action. completion-storm does `sample → refresh counters` every 500 ms (`apps/completion-storm/app.contract:216-220`).
  - It misses mutations sent by a timer. Messages sends `synchronize → change` every 3 s (`:472-475`), the reply feeds `revision`, and `revision` is an argument of all six resources (`:88-104`). It also misses the declared `refreshes` that such a send forces.
  - It misses chains from one resource to another: Weatherlight's `outlook(forecast, …)` (`apps/weatherlight/app.contract:102`) and Bluesky's `loads = loaded(homeFeeds, authorFeeds)` (`app.contract:91`).
  - It treats a conditional write as a periodic one. RealWorld's `settle` declares `writes nav` (`:197`), and `nav → current` feeds every resource's arguments (`:105-134`). So the line would print "tags, feed, post, thread, person (settle, every 100 ms)", which is false in practice.
  - It prints the timer's period, not the real re-ask rate. Weatherlight writes `revision` once every 600 ticks (`:140-144`) but would print "every 1000 ms". A `now()` dependency is re-evaluated at every commit, taps included, not only on one timer.
  - As specified, it gives a wrong line for RealWorld and Weatherlight and no line for completion-storm.
- **MATERIAL: `asks` is already in `logs`.**
  - Every time a source is asked for a resource, the runner journals `query <resource>: <source>`. This happens at the only call site, `self.query` (`runner/src/runner/settlement.rs:484`), which logs at `runner/src/runner.rs:1132` (`lines.rs:37`). Host facts are excluded.
  - Replies are journaled too, with their byte counts (`fulfil n (x) [HTTP 200, N bytes]`, `lines.rs:82`).
  - An answer's size can already be computed from `state.resources`, which carries the full value (`agent.rs:509-520`).
  - So "a minute of idle" is answered today by `clock +60000 logs` plus counting lines.
- **MATERIAL: `asks`/`rows` as specified don't fit and measure the wrong thing.**
  - "Each resource gains `asks`, `rows`" can't happen as written. A resource's entry in `state` *is* its typed value, which can be a list, so fields can't be added to it. It would need a sibling section, the way `pending` was added (LLP 1012:327). Changing the value shape would also break `agent.mjs:1383`, which merges `state.resources` into its lookup bag.
  - `rows` counts the outermost list. Bluesky's `homeFeeds`, `authorFeeds` and `threads` are `list<Feed>` (`app.contract:76,89,95`), so a 600-post feed would read `rows: 1-3`. That is exactly the case P4 is about.
- **MINOR: citation.** The cost line is printed at `contract/cli/src/build.rs:135`; `map.rs:104` is `component_costs`.

**Rules and order**
- **MATERIAL: the working set is over its cap.** `llp/current/` has 19 entries against the cap of 15 (`rules/RULES.md:12`, a **[check]** rule). This lane took it from 15 at `HEAD~2` to 17 and then to 19. `caps` should fail at this commit. I couldn't run it because `bun` isn't installed here.
- **MATERIAL: §4's order causes two format bumps.** P2 lands in step 1 and P1 in step 3, which means two `FORMAT_DIGEST` bumps. The digest covers the whole `format.json`, stdlib table included (`plan/build.rs:250-265`). That contradicts §4 step 3 and .005 §Costs.
- **MINOR: §6 names no real take.** Withdrawing `touching`, an unaccepted draft, and renaming `formatClockTime` are not takes (`rules/NOT-DOING.md:348-351`). Admitting Bluesky is a consumer expansion and needs a real one.

### Suggestions
- Add to §1: the polling `settle`, Messages' `now()` argument, and view state passed as arguments.
- Cut P6's counters. Document the recipe instead: `logs` for asks, `state` for sizes.
- Cut P6's static line, or narrow it to chains written *unconditionally* by a timer. That can be decided on the compiler's AST. It must include `refresh`, `send` and its `refreshes`, mutation reply slots, `now()`, and resource-to-resource chains. It should say "may be re-asked", with no period.
- Merge P3 and P4 into one decided feed design, gated on §5's measured residual.
- Fix the RealWorld success test.
- Get `llp/current/` down to 15 or fewer before anything lands. 1054.000.000 has landed (1054.000:29), so it is a candidate to archive.

### Verdict
**BUILD WITH CHANGES.** Build P2, P5 and a smaller P1. Cut P6 or rework it. Defer P3 and P4 until they are one design.

---

## LLP 1054.000.003 r2: Dates, numbers and durations in the roster

### Assessment
r2 fixes most of what r1's reviews raised: the derive-based clock, `NaN` from `calendarDiff`, the 1–9999 range, the policy markings, the corrected DST direction, and the wrapper limitation. The Intl outputs it quotes are right (checked below). But D8's linking mechanics would break Caltrain's bake and every native host. The scope still runs past its fixtures, and in two places it contradicts its only consumer.

### Concerns
- **BLOCKER: D8's seam can't be filled where the runner boots.**
  - D8 puts `exact-format` in a crate above the runner and says the runner's `Cargo.toml` won't name it. Then `RunnerLinks::ALL` (`runner/src/runner.rs:352-356`) can't point at it.
  - `ALL` is what these boot with:
    - Apple and Linux (`host/apple/src/host.rs:325` and `host/linux/src/host.rs:136`, both through `runner/src/runner/delivery.rs:44-67`);
    - the render host (`host/render/src/lib.rs:125,157`);
    - **the bake** (`contract/cli/src/lib.rs:516`);
    - about 89 files that call `Runner::boot`.
  - There are no "Apple and Linux entries" that fill runner links: native linking by use comes after the web target (LLP 1047 §10, answer 5). The only admission check is on the web (`host/web/src/link.rs:120-127`).
  - The result: Caltrain (`render=build`, `app.contract:9-11`) would trap during its bake with `Trap::TypeMismatch` (`runner/src/vm.rs:625-626`). That is a runtime gap, which LLP 1047 D6 forbids, not a named refusal.
  - **The fix is the router precedent.** The runner already depends on `exact-route` (`runner/Cargo.toml`) and reaches it only through a function pointer. The web registers it from `host/web-capabilities/src/router.rs:8-11`, and the linker drops it when unused. Do the same for format:
    - a `format` field in `exact_web::Linked` (`link.rs:19-39`), counted in `uses()` (`:54-75`) and passed through `runner_links()` (`:111-117`);
    - `Capability::Format` in `uses.rs:20-49`, where `ALL` is sized `[_; 6]`;
    - a Call-operand scan, for which `vm::instructions` (`vm.rs:362`) and deps' `Now` scan (`deps.rs:351`) are precedents;
    - a new `exact-web-capabilities` module.
  - The Costs section lists none of these.
- **MATERIAL: one function pointer links all six entries.** An app that calls only `formatTime` links relative time, compact numbers and durations too. That app is Caltrain, the v1 app, and RealWorld would be next: it is LLP 1047's byte bench. Either give each entry its own pointer, or measure against 1047's tap-at-load goal before RealWorld adopts `formatDate`.
- **MATERIAL: the fixture rule (`format.json` `_stdlib`) isn't met.**
  - `formatRelativeTime` has **no caller**. Bluesky keeps its bare `"5m"` as an app `fn` (D3, line 155), and Typetour's "12m ago" is fixed text (`apps/typetour/app.contract:72-74`).
  - The `"short"` date has no fixture. Messages holds only "9:41 AM", "Yesterday" and "Monday" (`apps/messages/app.ts:10-15`).
  - `formatDuration`'s `digital` and `narrow` styles have no fixture, and no app uses `timeupdate`.
  - `calendarDiff "day"` and the `"timer"` duration are one-line `fn`s today. My probe `/tmp/rv56/dur.contract` builds both for +302 plan bytes in total.
  - r2 misreports the reviews. It says local calendar days "need civil-from-days arithmetic" (lines 85-87). The Claude review gave a one-line `floor` difference, and Astra's probe was 405 bytes. The 1.9 KB figure was for a full date formatter.
- **MATERIAL: D5 sets up a false choice between Intl and Bluesky on compact numbers.**
  - Intl can truncate. `{notation:"compact", roundingMode:"trunc"}` gives `999 1K 1.2K 1.9K 12K 99K 999K 1.2M 1B`, which matches Bluesky's `count` (`data/src/fmt.rs:188-209`, "as the official client prints it") exactly.
  - The RFC's rounding choice prints 999,999 as `1M`.
- **MATERIAL: `-0` and rounding differences.**
  - `"integer"` prints `-0` for inputs in (−0.5, 0), and `"grouped"` prints `-0` for −0 (checked with Node).
  - Weatherlight uses `Math.round`, which prints `0°` and rounds −2.5 to −2 (`apps/weatherlight/app.ts:101`).
  - Use Intl's `signDisplay:"negative"`, or declare the difference. The Weatherlight "migration" also means rewriting an all-string `Outlook` resource, which is not small.
- **MINOR: the example doesn't compile.** `when` is a reserved word; `fn when(…)` gives `syntax-expected-name` (probe `/tmp/rv56/when.contract`). D9's example has the same problem.
- **MINOR: units and sign.** `formatDuration` takes milliseconds, but media events report seconds (`host/web/media-glue.js:63`). The sign of a negative sub-second input is unspecified.
- **MINOR: stray claim.** "1.005 at two digits" is correct Intl behaviour, but no style has two fraction digits.
- **MINOR: absence test.** Say that the "artifact check" extends the existing asynchronous absence report and is not a new check (RULES §Agents).

### Suggestions
- Fix D8 by following the router precedent.
- Scope for now:
  - `formatTime "short"`, replacing `formatClockTime`;
  - `formatDate "long" | "medium" | "month-day"`;
  - `formatNumber "grouped" | "compact"`, with `roundingMode:"trunc"`.
- Document relative time, calendar days and `m:ss` as `fn`s in the authoring notes. Add them to the roster when a consumer actually calls them.
- Keep a single digest bump shared with `trim`.

### Verdict
**BUILD WITH CHANGES.** The D8 mechanics must be fixed first, and the scope cut to what has fixtures.

---

## LLP 1054.000.004 (withdrawn): is the reasoning right?

Yes. Every stated reason checks out:
- A like adds rows to the Likes tab and to liked-by (`reads.rs:321-325, 692-696`).
- A favorite adds a row to the Favorites feed (`app.ts:88`).
- A failed delete's rollback loses its refresh (`writes.rs:157-161`, `reads.rs:70-74`).
- `Instruction.args` holds only three operands (`vm.rs:323-332`).
- F5's cost figures were stale.

One overstatement: "can't go stale" is too strong for the replacement (see P5 above).

---

## LLP 1054.000.005: `trim` in the roster

### Assessment
It is correct, small and well tested. The whitespace set matches JavaScript exactly: a Node sweep of every code point finds the same 25. Its one real error works in its favour: an **admitted, in-repo** fixture exists.

### Concerns
- **MATERIAL (it resolves the open question): the fixture is Fieldnotes, not only Bluesky.**
  - Fieldnotes' Save button is disabled on `(title == "" and body == "")` (`apps/fieldnotes/app.contract:256`).
  - Its source throws on whitespace-only input (`apps/fieldnotes/app.ts:144`, "Write something before saving.").
  - So a note made of spaces enables Save, and the tap fails.
  - Messages' `canSend` (`app.ts:416`), together with `contacts` taking `newDraft` (`app.contract:102`), is a second in-repo case, and a performance one.
- **MINOR: format bumps.** Don't hold `trim` back for P1. A second bump before 1.0 costs less than waiting.

### Suggestions
Make Fieldnotes' Save button the driven fixture. Optionally, move Messages' `canSend` into the view and drop `newDraft` from `contacts`.

### Verdict
**BUILD.**

---

## LLP 1054.000.006: Arguments on list edges

### Assessment
The mechanics are right and it really is about 10 lines.
- **"No plan format change" holds.** Every handler row already has an `args` range (`format.json` `handlers`). Nothing validates handler arguments against the event at the plan level (`plan/src/lib.rs:220-300`). `dispatch_edge` (`runner/src/runner/event.rs:640-671`) only has to do what `dispatch` already does (`:779-783`).
- **The rule is confirmed today.** `reachend=more(f.id, f.cursor)` is refused with the confusing message "takes 2 parameter(s); `reachend=` supplies 2" (probe `/tmp/rv56/edge.contract`).
- **Its need is conditional.** It matters only for lists inside `each`.

### Concerns
- **MATERIAL: the example promises more than it delivers.** "`window = cursor` // or a per-feed map" can't hold more than one window, because Contract has no maps and no indexing. Bounded answers across many lists still need the windows held in the source, moved by a mutation plus a declared refresh. And a single list doesn't need arguments at all (`apps/messages-stress/app.contract:75-80`).
- **MATERIAL: consumer.** The only consumer is Bluesky, which isn't admitted (as .006 itself says in §Costs).
- **MINOR: add a test for an argument that fails to evaluate.** The edge should be re-armed, as the existing error path does (`runner/src/runner/collection.rs:163-167`).

### Suggestions
Land it together with P4's decided feed design and a working pattern for a window per feed in the source.

### Verdict
**BUILD WITH CHANGES, and deferred** until Bluesky is admitted and P4 settles on a `list<Feed>` design.

---

## Rules
- **Consumer bar.** P2 meets it through Fieldnotes, and P5 through RealWorld. P1 meets it only for `formatTime` (Caltrain), and arguably for RealWorld's dates. P3, P4 and P1's calendar and compact parts depend on Bluesky's admission.
- **Fixture rule for roster entries.** See .003 above: relative time, the short date and two duration styles fail it.
- **Apparatus.** None of the plan adds a check, script, registry or config file. The new design documents need Charlie's approval, which he apparently gave on 2026-09-26. The working set at 19 against 15 breaks a [check] rule.
- **Syntax freeze.** No new syntax. P3 relaxes an arity rule, and D9 is a type diagnostic.
- **One in, one out.** Nothing is moved off NOT-DOING. Admitting Bluesky is an expansion and needs a named take, and §6's takes don't count.
- **NOT-DOING's "dataflow" and "perf", and the eight operations.** The static line isn't the refused dataflow explorer as long as it stays one line with no "why" chain. Counts in `state` are not the refused `perf` operation: there are precedents in `pending` (LLP 1012:327) and the game's `world.perf` (`host/web/gpu-glue.js:572`), and no ninth operation is added. The counters are simply unnecessary, as shown under P6.

## Order and scope
§4's order is wrong on four points:
- P3 comes before any feed design exists.
- P2 and P1 cause two digest bumps.
- P6's counters come first, but they duplicate `logs`.
- P4 is committed to before any measurement.

**Cut:** P6's counters; P6's static line unless it is made precise; `formatRelativeTime`, `calendarDiff`, `formatDuration` and the `"short"` date.
**Missing:** the polling-for-replies pattern (a hook that runs when a reply lands deserves its own LLP later); Messages' `now()` argument; the list of what D8 has to touch; and fixing the working-set cap.

## Open questions
- **1056 Q1 (admit Bluesky).** It isn't needed for P2, which has Fieldnotes. Admit it only if Charlie wants the port to drive P1's calendar and compact parts and P3 and P4, and then name a real take.
- **1056 Q2 (`asks`/`rows` in `state`).** Acceptable in principle, but not needed. Asks are the `query` lines in `logs` (`runner.rs:1132`), and sizes come from `state.resources`. If it is added anyway, use a sibling section, count calls to the data source only, and report bytes, not the outermost row count.
- **1056 Q3 (Caltrain's boards).** Keep them as they are. The view can filter with `when d.at >= nowMs`, but without indexing it can't limit the list to N rows. The boards are cheap Rust and the v1 app works.
- **.003 Q1 (enough consumers?).** Not for six entries. The in-repo fixtures justify `formatTime`, some `formatDate` styles and `formatNumber` (with truncating compact). Messages and Typetour hold fixed text only.
- **.003 Q2 (future timestamps).** Agree with `"now"`, marked as exact2's policy.
- **.005 Q1 (is Bluesky's fixture enough?).** Yes, and it doesn't matter, because the fixture is in-repo in an admitted app (`apps/fieldnotes/app.contract:256` against `app.ts:144`). No trade is needed.
- **.006** has no open questions. Its implicit question, its consumer, is answered by deferring it.

## Bottom line
Implement in this order:
1. Get `llp/current/` down to 15 or fewer.
2. **P5 in RealWorld.** It needs no exact2 code. Measure before and after by counting the `query` lines in `logs`, and use "asks `feed` and `post`, one GET" as the test.
3. **P2 `trim`, with Fieldnotes' Save button as the fixture, plus the smaller P1, in one format change.** P1 is `formatTime`, `formatDate` long/medium/month-day, and `formatNumber` grouped/compact with truncation. It is linked through the router pattern: a runner dependency plus a `RunnerLinks` pointer that `ALL` fills and that the web registers through `exact-web-capabilities`. Document relative time, calendar days and `m:ss` as `fn`s.
4. In the Bluesky repo, adopt P5 and the smaller P1, then measure what is left.

Only if that residual justifies it, write P3 and P4 as one feed design. Drop P6's counters, and rebuild the static line precisely, or drop it too.