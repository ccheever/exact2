# Panel: LLP 1056 with 1054.000.003 r2, .004 (withdrawn), .005, .006, 2026-09-26 (grok)

- **Family:** xAI, Grok 4.7 at `--reasoning-effort xhigh`, Grok CLI 1.0.41, `--always-approve --max-turns 400 --output-format streaming-messages-json`. Run in a detached checkout at `5a303070` (`/tmp/grok-wt`), so that its read-only shell checks couldn't touch the working tree. 51 turns.
- **Brief:** the shared panel brief (build / build with changes / don't build per part; verify against code; rules; order; open questions).
- **Redacted:** no. The final message follows verbatim.

---

The withdrawal of `touching` holds, and the plan is worth building only with the seam, feed, and report defects below fixed. P3 is the piece that is ready as written.

## LLP 1056 — The fast path by default

### Assessment

The mechanism story is right: one seam, whole-answer re-asks, and the cheap pattern buried in LLPs. The numbers check. F5's current rows are 1.40 ms (path-only stringify) and 0.72 ms (direct decode) per 1,000 synthetic rows (`llp/1027.003-value-transfer-and-shared-storage.research.md:143-147`), not the old 3.75 ms. Bluesky really does pass one minute clock into eight resources plus `unread` and `hasNew` (`app.contract:42-49, 76-134, 180`), `change` really refreshes nine (`:58`), and `Post` really has 38 fields including `time`, `likes`, `lineAbove`, and `role` (`shapes.contract:74-112`). r2 and this plan do answer the r1 reviews: the `fn` alternative is weighed, styles without fixtures are cut, the blank-for-30s slot is a derive, `touching` is withdrawn for the reasons both reviews gave, and C5 is its own RFC. What does not survive is P4 as specified, the certainty of P6's static line, and P1's description of the link seam.

### Concerns

1. **BLOCKER — P4 cannot be written as specified.** "One resource per mounted list" (`1056:175-177`) is not a Contract program when the set of lists is data. Resources live on the root (`type-child-resource`), and there is no list indexing (1054 L5). Bluesky's home screen is one resource, `homeFeeds … as shape list<Feed>` (`app.contract:76`), drawn as `each f in feeds` with a list per feed (`screens.contract:240-244`). A window move re-asks that one answer. Returning only the selected feed's window (other feeds as stubs) bounds the crossing and needs no new runtime. The plan forbids the runner from windowing automatically, which is correct (`1056:179-181`), then prescribes a split the language cannot say.

2. **MATERIAL — P6's static line is a may-analysis and misses real timer asks.** Argument dependence is real and already scanned: `LoadSlot`, `LoadDerive`, `Call now` (`runner/src/instance/deps.rs:330-363`). It is not "this resource is re-asked every interval."
   - Any timer sets `now_ms` before the action (`commit.rs:203-208`). A derive that floors `now()` changes value on its own cadence, not on every other timer.
   - A timer action's `refresh`, and the `refreshes` of a mutation it `send`s, re-ask with the arguments unchanged (`commit.rs:417-424`). Messages' `tick` does this (`apps/messages/app.contract:464-468`). The specified chain never looks at those opcodes. Both are in the plan tables (`format.json:295-309, 186-214`).
   - A store-reading resource misses reuse when the revision moves (`settlement.rs:387-403`). A timer that writes the store is invisible to this chain.
   - Dependence through another resource's value is invisible too. Closing over `LoadResource` is the conservative fix.
   - RealWorld's `settle` is `every(100, …)` and declares `writes nav` (`app.contract:197-219`). `feed` depends on `nav` through `current`. The line as specified reports `feed` re-asked every 100 ms. The slot usually does not change, so the asks counter will not move. That false line is the app P5 is supposed to be measured on.
   - The parenthetical `(tick, every 1000 ms)` states a rate. For Caltrain it happens to be tight (`nowMs = nowMs + 1000`, `app.contract:70-71`). It is not the analysis.

3. **MATERIAL — `asks` / `rows` are the right question and the wrong encoding.** `state` prints each resource as its typed value (`runner/src/agent.rs:509-521`). A test requires `"northBoard":[{"id":` (`host/web/tests/it/agent.rs:78-80`). Putting `asks` and `rows` on that value wraps it and breaks 1012's "resources … as typed JSON." Sibling maps (`asks`, `rows`) leave the value alone. Count every `query()`, including a send-time reread whose `Later` is discarded (`settlement.rs:502-511`): that is the waste P5 removes. Do not count a compiled boot hit (`:423-434`); Caltrain's "60" (`1056:266-267`) is right only because the boot answer is baked. `rows` = length of a list answer, or of the one list field of a record. `homeFeeds` is a `list<Feed>`, so that number is the feed count, not the posts. Say so, or the before/after on a 20-page Bluesky session measures the wrong thing.

4. **MATERIAL — P5's RealWorld demo line is wrong, and the settle tick is load-bearing.** `favorite` on the article page must refresh `post` as well as `feed`. On the home page `article("")` returns immediately (`app.ts:106-107`), so the network win is dropping `thread` and `person`, not "asks `feed` only" (`1056:256-257`). `follow` changes which articles the following feed holds (`app.ts:88-95`) and must refresh `feed` and `person`. Publish and delete navigate off one `change` slot (`app.contract:212-217`). Bluesky's `settle` toasts off the same single slot (`app.contract:476-479`). Splitting the mutation without retargeting those ticks drops the redirect and the error toast. No runtime change is needed. The app change is bigger than the RFC says.

5. **MATERIAL — P1's "links none of their code" is true of web production only, and not by the seam written.** As built, `RunnerLinks` is `surface_answer`, `router`, `lists` (`runner.rs:333-339`). Native boots `RunnerLinks::ALL` (`:330`, `host/apple/src/host.rs:325`). 1047 D8 (native link set as a compatibility input) is explicitly later. Web links through `exact_web::Linked` and `linked!` (`host/web/src/link.rs:19-39`, `host/web-capabilities/src/lib.rs:25-43`), filled by `web_linked` (`contract/cli/src/logic.rs:63-95`). `stdlib::call` does not take that table (`stdlib.rs:14-20`, `vm.rs:625`). There is no Apple or Linux entry that fills a `format` pointer from the use-set. Copy the web router/markdown seam. On Apple and Linux the code stays in every binary until D8, same as the router. `uses()` must be a scan of `Call` operands across the code blob. `fn`s are expanded at the site (`contract/lower/src/expr.rs:195-236`), so a scan of the blob sees them.

6. **MINOR — §1 is a little hard on Caltrain and a little loose on "eight list resources."** Caltrain's `nowMs` filters boards and feeds two canvases (`app.contract:63-64, 257, 271`). The re-ask is real. It is not a label clock. `session(clock, …)` (`bluesky app.contract:49`) is one of the eight and is not a list. The 72–112 ms / 6.4 ms figures match 1054 §7. The Rust-vs-TS authoring split matches `DIARY.md:46-52`.

7. **The `.004` withdrawal is right.** A like adds a row the current answer does not contain: the viewer's Likes tab (`reads.rs:323-327`, `getActorLikes`) and liked-by (`reads.rs:694`, `getLikes`). A favorite can insert into `favorited=username` (`app.ts:88-94`) when that article is absent. A failed delete clears the overlay (`writes.rs:220-227`) after the send-time read has already filtered the post out (`writes.rs:159`, `reads.rs:72-76`), so a reply-time walk misses it and skips the restore. Membership and rollback are not in the committed value. Splitting by kind of write cannot go stale. `Send` is three operands (`format.json:797-802`, `vm.rs:661-674`). Leave `touching` withdrawn.

### Suggestions

Rewrite P4 as: a paged resource's arguments are the window, and a multi-list screen answers the selected window (stubs for the rest) or a fixed set of named resources. Keep P6's counter. Make the static line say "may," and include `refresh` and the declared refreshes of `send`s. Land P2's stdlib row in the same `FORMAT_DIGEST` change as P1 (`bake/src/compat.rs:297-300`). §4 currently schedules P2 first and the shared bump third, which is two cohorts.

### Verdict

**BUILD WITH CHANGES.** Do not implement P4 or the P1 seam or the static line as written.

### Open questions

1. **Admit Bluesky?** Not by silence. Admit it if relative time, `calendarDiff`, compact counts, and the feed work are in this round. Do not block P3, Messages' `trim`, Caltrain's clock rename, or the RealWorld mutation split on that decision.
2. **`asks` / `rows` versus "perf"?** Acceptable. NOT-DOING's eight operations are `tree · screenshot · tap · type · state · layout · logs · clock`, and a new question is answered from `state` (`rules/NOT-DOING.md:295-302`). Two integers on the existing reply are not a ninth operation and not a profiler. The static line is not the refused dataflow explorer if it stays one summary, like the node-cost line (`contract/cli/src/build.rs:125-138`). A witness, a graph, or a timing breakdown would cross `NOT-DOING.md:304-316`.
3. **Caltrain.** Leave the boards on `nowMs`. Report them. Do not move filtering into the view in this plan.

## LLP 1054.000.003 r2 — Dates, numbers, durations

### Assessment

r2 clears the r1 blockers. Invalid `calendarDiff` is `NaN`, and `==` is IEEE so `NaN` matches nothing (`runner/src/compare.rs:9-10`, `vm.rs:562-566`), so the chain falls through to a formatter that returns `""`. The clock is a derive, so it moves when `exactTime` arrives rather than 30 s later. The quoted `en-US` strings match Node 26.9.0 / ICU: `Sep 26, 2026`, `1:02 PM` with U+0020, narrow `5m ago`, `numeric: "auto"` of 0 seconds as `now`, compact `1,250 → 1.3K`, percent `0.425 → 43%`, integer `2.5 → 3` and `-2.5 → -3`, digital `0:03:05`. The fixed-offset DST wording now points the right way, and ±1,080 minutes matches `WallTime::validate` (`time.rs:26-30`). The `fn` alternative is honestly demoted to plan bytes. What is left is the seam, a few fixtures that are not call sites, and `formatDuration`.

### Concerns

1. **MATERIAL — The link seam is not 1047 as built.** Same defect as 1056 concern 5. `Capability` is six variants (`uses.rs:20-38`), none of them `format`. Web admission is `Linked` plus `linked!`, not a field beside `router` on `RunnerLinks`. Native will keep the code in every binary until D8. Say that. Measure web raw and brotli for an app that calls it (Caltrain) and one that does not (video player), as the RFC already plans.

2. **MATERIAL — Several "fixtures" are not call sites.** Typetour's `"12m ago"` is a literal prop (`apps/typetour/app.contract:72`). Messages' `"Yesterday"` / `"Monday"` are fixture strings (`apps/messages/app.ts:10-15`). Weatherlight does `` `${Math.round(h.humidity)}%` `` on a 0–100 quantity (`app.ts:111`). Intl `style: "percent"` multiplies by 100, so 42 becomes `4,200%`. `integer` is half-away-from-zero (`-2.5 → -3`); `Math.round(-2.5)` is `-2`. RealWorld's date (`app.ts:23-25`) and raw `favoritesCount` (`app.contract:447`) are time-invariant. The only in-repo time formatter today is Caltrain's `formatClockTime` (`app.contract:278, 320`). Bluesky's own `ago` is bare `"5m"` and `"now"` below 5 s (`fmt.rs:112-132`), which this RFC correctly leaves as an app `fn`, and `month_year` (`fmt.rs:169-188`) has no style. The sample `when` emits `"5m ago"`, which is not the port's string.

3. **MATERIAL — `formatDuration` has no view that formats a position.** The in-repo player uses native controls. `"timer"` is exact2 policy, tested against a table, which is fine, but the roster rule is "by fixture, never by speculation" (`format.json:823`). Zero components are dropped: `DurationFormat` narrow of 1 h 0 m 5 s is `"1h 5s"`. All-zero narrow is `""` unless `secondsDisplay: "always"`, which the RFC does specify and which is `"0s"`. Ship duration when a view owns a playhead.

4. **MINOR — The prose still says "1.005 at two digits is 1.01."** No listed style has two fraction digits. Grouped (max 3) prints `1.005` on this Node. Keep it as an oracle row only if a style shows it. `formatClockTime(NaN)` and `formatClockTime(-0.5)` really do differ (`stdlib.rs:114-116`); Intl throws on `NaN` and prints `12:00 AM` for `-0.5`. D10 is an intentional behavior change. Good that it says so.

### Suggestions

One capability, web-linked like markdown. In the same change, migrate Caltrain's two `formatClockTime` calls to `formatTime(t, 0, "short")` and move the `diagnostics.rs:842-843` hints. Add `formatDate` `"long"` and `formatNumber` `"grouped"` only if RealWorld's templates change in that change. Hold `"compact"`, `formatRelativeTime`, `calendarDiff`, `"percent"`, and `formatDuration` for a real call site. The literal-style check and the "wrapper `fn` cannot forward a style" limit (`types/src/lib.rs` shadowing at `:625`, precedent `routes.rs`) are right. Keep them.

### Verdict

**BUILD WITH CHANGES.**

### Open questions

1. **Are the in-repo fixtures enough, with the 2026-09-26 request?** Enough for `formatTime` `"short"` (Caltrain, a rename) and, if migrated in the same change, RealWorld's long date and grouped counts. Not enough for relative time, calendar diff, compact, percent, or duration. Those wait on an explicit Bluesky admission or another call site.
2. **Future timestamps as `"now"`?** Yes. That is Bluesky's behavior (`fmt.rs:117-120`), and `"in 3s"` is a worse feed label. Keep it marked as policy, not Intl.

## LLP 1054.000.005 — `trim`

### Assessment

The entry is the right size and the right character set. On Node 26.9.0, `String.prototype.trim` strips the 25 code points listed and keeps U+0085, U+200B, and U+180E (U+0085 is kept: the trimmed string still has length 3). Rust `char::is_whitespace` does the opposite on NEL versus FEFF, so a match, not `str::trim`, is required. A `fn` cannot walk characters (1006 §3). Core rather than a linked capability matches `length` and `contains`. Same-`Rc` when nothing is trimmed is a good hot-path detail.

### Concerns

1. **MATERIAL — The named fixture is out of repo, and the in-repo one is stronger than the RFC says.** Bluesky's two buttons are real and unadmitted. Messages already has the bug and the cost: `recipients(recipient, recipientQuery, newDraft, …)` (`app.contract:102`) and `canSend: … && !!body.trim()` (`app.ts:416`). `newDraft` is not used for anything else in that function, and the composer writes it on every keystroke (`app.contract:447-448`). The button reads `contacts.canSend` (`new-message-sheet.contract:248`). Moving the trim into the view and dropping `newDraft` from the arguments stops a source ask per keystroke. A synthetic Contract test is not the fixture rule (`format.json:823`).

2. **MINOR — Grapheme length is correctly a finding, not an entry.** `length` is UTF-16 (`stdlib.rs:70-71`). Segmentation tables do not belong in core. Leaving it to the source is right.

### Suggestions

Ship `trim` in the same `FORMAT_DIGEST` change as whatever `.003` entries survive, and migrate Messages' `canSend` in that change: the source answers "the addresses resolve," the view says `trim(newDraft) != ""`. That is the admitted consumer. NOT-DOING does not refuse `trim` (`:348-351`), so no doing-list trade is required.

### Verdict

**BUILD WITH CHANGES.** The change is the Messages migration, not a new design.

### Open question

Bluesky's buttons alone are not enough. Messages is, if this change updates it. If neither lands, don't add the row.

## LLP 1054.000.006 — Arguments on list edges

### Assessment

The "no plan format change" claim is true. `handlers` already has `args: range:args` (`format.json:431-445`). Lowering already compiles handler arguments and stores them (`contract/lower/src/lib.rs:1438-1444`). `dispatch` already evaluates them in the view's frames (`event.rs:782-785`). `refresh` is a host event and takes that path today. The only refusal is `handler_arity` returning `None` unless `given == 0` for `reachstart`, `reachend`, and `refresh` (`contract/analyze/src/lib.rs:437-440`), which lowering shares (`lib.rs:1392`). `dispatch_edge` passes `Vec::new()` and says the events have no authored arguments (`event.rs:638-667`). Changing that function to evaluate `handler.args` the way `dispatch` does, at fire time, is the whole runtime change. Deferred edges wake by bumping the collection epoch (`collection.rs:180-191`, `traversal.rs:259-266`) and dispatch later, so arguments are read then, not when the edge was armed. Types of curried action arguments are already checked (`contract/types/src/lib.rs:655-677`). This is what removes `loaded(homeFeeds, authorFeeds)` and the `*Asked` guards (`bluesky app.contract:91, 229-243`).

### Concerns

1. **MINOR — Two tests, not one.** `collection_edges.rs:478-484` refuses `reachstart=change(0, 2)`. `action_related.rs:160` asserts `handler_arity("reachstart", 1)` is `None`, and the shared table at `:141-142` expects payload 0. Both move. The RFC names only the first.

2. **MINOR — The consumer is still only Bluesky.** Messages-stress is one list and passes no arguments (`apps/messages-stress/app.contract:192`). Nothing else in `apps/` names `reachend`. The change is a relaxation of a refusal, not new syntax, and 1035.005 §1 step 4 is not in play. Adoption in the port still waits on admission. The code does not.

### Suggestions

Clone the handler (action id and arg codes) before `eval`, as `dispatch` does. `dispatch_edge` currently holds a reference across `run_action`. Add the two-list runner test the RFC describes: each list dispatches its own key, and a deferred edge sees the cursor at dispatch.

### Verdict

**BUILD.**

## Rules

The plan does not reverse a NOT-DOING entry, and it does not add syntax. One-in/one-out is not triggered. The fixture rule and the consumer bar are. Roster rows are "by fixture, never by speculation." A Node oracle table is a test, not a fixture. Bluesky is not an admitted consumer (`1054.000:244-247`). P6's line and counters are not a new check, script, or registry (`RULES.md` agents: add no apparatus). Do not add a blocking metric. `llp/current/` already has 19 links against a cap of 15 (`RULES.md`). Building this should not add another. Archive before adding.

## Bottom line

Implement now, in this order: **(1)** `.006`, the edge-argument relaxation, no format bump. **(2)** P6's `asks` and `rows` as sibling fields on `state`, counting every source `query` and defining `rows` as the list you mean. **(3)** P5 in RealWorld only: `favorite` refreshes `feed` and `post`, `follow` refreshes `feed` and `person`, comments refresh `thread`, publish and delete keep the `settle` redirect, then read `asks` on one home-feed favorite. **(4)** One `FORMAT_DIGEST` change containing `trim` (with the Messages migration), `formatTime` replacing `formatClockTime`, and `formatDate` `"long"` / `formatNumber` `"grouped"` only if RealWorld's templates move in that same change. Link `format` on the web the way markdown is linked. Leave it in every native binary until 1047 D8. Hold relative time, `calendarDiff`, compact, percent, and duration for an explicit Bluesky admission or another real call site. **(5)** Do not build P4's "one resource per dynamic list," and do not build `touching`. After Bluesky is admitted, bound each feed by answering the selected window from the one resource the language can declare, and split `mutation change` the same way as RealWorld, including `settle`.