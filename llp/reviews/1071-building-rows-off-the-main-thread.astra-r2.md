# Review: LLP 1071 r2's retirement handshake, 2026-09-28 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort max; a focused re-review of r2 §5 (the retirement handshake and `CollectionFeedback` v4), commissioned by the coordinator session. Transcript kept by the coordinator (`scratchpad/offmain-review2.out`).
- **Reviewed revision:** LLP 1071 r2 (commit `830f17a0`).
- **Redacted:** nothing. Local file links point at the `exact2-wt-mount` worktree.
- **Verdict:** go with named changes; not ready for stage 2.
- **Disposition:** LLP 1071 §0.2 (r3). The handshake is replaced by create-only asynchronous fills with synchronous eviction, timer advances and edge actions (§5), chosen on the coordinator's suggestion after measuring eviction's share (6.5 ms/s, about 0.6 ms a row). That makes #1–#5 and the P2 moot, answers #6 with a `mode` word plus an ABI version bump, and answers #7 by keeping destructive commits synchronous.

## The review, verbatim (final message)

**Verdict: go with named changes.** Keep the owner-thread direction. The retirement handshake is not complete enough for stage 2: it can still produce `UnknownView`, and the promised synchronous parity and v4 compatibility need explicit rules.

1. **P1 — The acknowledgement can recreate the original race.**  
   §3.1 permits an asynchronous fill to carry acknowledgements. Sequence: main acknowledges row R; the owner destroys R; that batch awaits application; UIKit still accepts a touch on R. The event again reaches [the `UnknownView` path]( /Users/ccheever/projects/exact2-wt-mount/runner/src/runner/event.rs:789). “Untouched since the fill was posted” does not protect the interval **after acknowledgement was sampled**.

   **Change:** make final retirement synchronous from a fresh main-side interaction check through destroy-batch application, or establish an explicit main-side withdrawal barrier that prevents new targeting and accounts for already-captured events before acknowledging. Hold the **acknowledgement’s destroy batch**, as well as the original fill, in the race tests. [RFC §5](/Users/ccheever/projects/exact2-wt-mount/llp/1071-building-rows-off-the-main-thread.rfc.md:370)

2. **P1 — “Alive, out of the window” needs a topology contract.**  
   Keeping the Rust `Row` allocation alone does not preserve event delivery. [`Tree::find`](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/find.rs:81) follows kernel parent links and looks up collection wrappers in mounted rows. [`emit_children`](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/views.rs:147) constructs children from mounted rows; UIKit’s [`placeChildren`](/Users/ccheever/projects/exact2-wt-mount/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:822) detaches omitted views. Removing a retiring row from either structure can break lookup or native focus/composition before its destruction is acknowledged.

   **Change:** specify retiring rows’ kernel parentage, native attachment, event lookup, and child/spacer geometry. Excluding them from measurement must not sever their event route or double-count their extent. Main must resolve interaction protection before any detach that would end the interaction.

3. **P1 — Reversal, deletion and key reuse need explicit transitions; revival also challenges parity.**  
   An untouched retiring row can become wanted again through reversal or `scrollIntoView`. An acknowledgement must not blindly destroy it. Conversely, an item disappearing from the data must not remain eligible for resurrection merely because its key later returns. Today [data removal and window eviction are distinguished](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/mod.rs:708), and remounting [initializes fresh row-local slots](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/mod.rs:814).

   **Change:** define a transition table: current window/jump/pins can cancel pending window eviction; data deletion invalidates that logical incarnation, its kept positions and its pending target. Qualify acknowledgements by runtime/list/view and retirement attempt, using existing revision machinery where sufficient. View IDs already are not reused; row keys and pooled object identities must not substitute for them.

   Explicitly resolve the parity consequence: reviving the old slots on Apple differs from immediate retirement and fresh slots on the synchronous path. Settlement alone cannot make those states equal.

4. **P1 — A historical touched set is not a complete pin model.**  
   Interaction may already be active when the fill is posted, especially VoiceOver focus or selection that the existing two fields do not capture. Conversely, a touch can begin and finish before `kept` is processed; promoting that historical touch to a pin leaves no future release event. The existing UIKit query checks [first responders only](/Users/ccheever/projects/exact2-wt-mount/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:157), and [selection callbacks](/Users/ccheever/projects/exact2-wt-mount/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:262) do not provide the proposed general selection/composition tracking.

   **Change:** distinguish current owners from interactions observed during the outstanding interval. Specify acquisition, transfer and release for each reason, including pointer cancellation and release after click. Reconcile simultaneous reasons with LLP 1070’s bounded focus/interaction chains; an arbitrary `kept: [view]` cannot silently become unlimited interaction pins.

5. **P1 — Nested retirement must be settled as a hierarchy.**  
   A kept inner row must prevent its retiring outer row from being acknowledged. Per-list acknowledgements processed independently can destroy the ancestor first. Current [collection traversal](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/traversal.rs:4) and [inner-pin discovery](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/nest.rs:168) visit mounted rows; both need defined treatment of retiring rows.

   **Change:** validate descendant keeps and ancestor protection before applying any retirement in that hierarchy. Final outer retirement destroys its remaining inner state atomically and invalidates queued child feedback. Incorporate the latest accepted inner position before [`keep_positions`](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/nest.rs:208); save on final window eviction, not retirement proposal, and never restore deleted-item state. An inner `scrollIntoView` must cancel the relevant pending ancestor eviction before resolving/building its destination.

6. **P2 — “Until the next accepted report” is not yet a residency bound.**  
   r2 does not specify how retirement acknowledgements force another report when geometry is unchanged or unavailable. Existing feedback has [deduplication and limited scheduling](/Users/ccheever/projects/exact2-wt-mount/host/apple/Sources/ExactKit/Collection.swift:390); the runner’s [window retention cap](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/mod.rs:727) does not automatically count a new retiring collection.

   **Change:** bound unresolved retirement cohorts—one per list initially—and require acknowledgement progress before admitting another beyond that bound. Retirement work must bypass geometry deduplication, survive stale retries, and participate in agent settlement. Count retiring descendants, heavy resources and queued batches alongside mounted rows and pins. Pool/reset only at final retirement, preserving LLP 1068’s incarnation rules; explicitly prevent settlement from unnecessarily constructing pending heavy leaves that are about to retire. The pool’s refusal to recycle a focused row does **not** prevent the ordinary destroy fallback.

7. **P1 — v4 needs an explicit synchronous mode and coordinated ABI delivery.**  
   The runner cannot infer deferred retirement from velocity or fill limit: synchronous hosts use those too. The web encoder [hardcodes v3](/Users/ccheever/projects/exact2-wt-mount/host/web/collection-glue.js:21), the shared decoder [accepts only v3](/Users/ccheever/projects/exact2-wt-mount/runner/src/instance/collection/api.rs:188), and Linux [constructs the shared feedback struct directly](/Users/ccheever/projects/exact2-wt-mount/host/linux/src/presenter/collection.rs:763). “Web and Linux: none” is therefore incorrect as implementation scope.

   **Change:** specify v4’s exact layout and immediate/deferred mode. Default synchronous callers—including `EXACT_FILL_SYNC`—must finalize eviction in the same operation, without an observable retiring phase or extra acknowledgement round. Update every encoder, decoder and typed caller together. Validate counts, identities, duplicate/conflicting acknowledgements and trailing bytes before mutation.

   Follow [LLP 1031 D2](/Users/ccheever/projects/exact2-wt-mount/llp/1031-brownfield-embedding.rfc.md:255): bump `EXACT_ABI_VERSION`, update the compatibility cohort, require matching ExactKit/archive artifacts, and test rejection in both mixed-version directions. Wire version 4 alone does not establish that packaging boundary.

8. **P1 — Other asynchronous mutations bypass §5 entirely.**  
   §7.1 makes timer advancement asynchronous while a fill is outstanding. [`advance`](/Users/ccheever/projects/exact2-wt-mount/host/apple/src/host.rs:837) commits application actions, which can delete a row, an inner list or an entire non-list subtree. Collection feedback also [dispatches edge actions](/Users/ccheever/projects/exact2-wt-mount/runner/src/runner/collection.rs:124), whose data updates use ordinary realization. Protecting only the report’s geometric eviction does not protect those removals.

   **Change:** apply the lifetime barrier to every destructive mutation executed asynchronously, or keep destructive timer/edge-action commits synchronous for this stage. Add held-publication cases where a timer or edge action removes the currently displayed interaction target; the original fill-only tests cannot establish safety.
