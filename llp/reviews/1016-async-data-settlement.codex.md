# Panel: LLP 1016 Asynchronous data settlement — §4, how a mutation is written (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only -C <capsule> --skip-git-repo-check --ephemeral -m gpt-5.6-sol -c model_reasoning_effort="ultra"`, the brief on stdin, workdir = the capsule (16 files: LLP 1016, 1004, 1005, 1006, 1007, 1012, `rules/RULES.md`, `rules/NOT-DOING.md`, `runner/src/runner.rs`, the caltrain and weird-castle contracts, weird-castle `data/src/lib.rs`, weird-castle `llp/0000`, `0004`, ibex LLP 0067, 0068), no network.
- **Method:** a panel at Charlie's request, 2026-08-30 (LLP 1016 §6: not a refine loop; no verdict binds). Round 1 mutually blind; round 2 each panelist sees the other's round-1 position verbatim. Brief (round 1) sha256 57b579c1f9fbfa777c497943d6e7f59f7e4593bd54e3a42e7389ba9885a53895; LLP 1016 sha256 fa8ed97e6c088c2d26d31d1f294fd821e4bd9c62313f2a9d2ce8e6da9f2873f3; capsule sha256 9af90cce34517544ec3907f13ab189e7112e3a2ae715084f4322f2c61ff96861.
- **Disposition:** the fold is recorded in LLP 1016 §4 and at the end of this file.

---

## Round 1 — position (received 2026-08-30T12:29:50Z, verbatim; run 2026-08-30T12:20:03Z → 2026-08-30T12:29:50Z; the run log confirms `model: gpt-5.6-sol`, `reasoning effort: ultra`)

## Position

**A: an explicit effect statement**, with a declared, typed result cell. Queries remain argument-keyed resources; effects run only when an action sends them. Every `send` creates a request even when its arguments equal an earlier invocation; it is neither memoized nor replayed after reload. The result cell retains its last settled value, exposes `pending(cell)`, and may be carried across a dev reload. A precise spelling would be:

```contract
shape Checkout
  url: string
  error: string

component WeirdCastle
  resource idleSession = idleSession() as shape Session
  resource idleCheckout = idleCheckout() as shape Checkout

  effect session = login as shape Session else idleSession
  effect checkout = createBrickCheckout as shape Checkout else idleCheckout

  action submit
    send session(who, password)

  action buyBricks(pack: string)
    send checkout(session.token, pack)

  view
    when pending(session)
      text "Logging in…"
    else
      when session.ok
        SignedIn(session=session)

    when pending(checkout)
      text "Preparing checkout…"
    else
      when checkout.url != ""
        link checkout.url label="Continue to Stripe"
          text "Continue to Stripe"
```

The two-step checkout is intentional: creating the Stripe session is the mutation; following its returned URL is ordinary browser navigation. It avoids adding completion callbacks before one-tap redirection is a demonstrated requirement.

## Why

1. **A mutation is an occurrence, not an argument key.** Submitting the same HTML form twice sends twice; calling `fetch` twice sends twice. React Query separates `useMutation().mutate(vars)` from keyed queries, and SWR separates mutation `trigger()` from resource revalidation. That is A. P makes an implementation detail—`attempt`—part of application state and cache identity. Login and checkout-session creation are not facts that become true merely because their arguments exist.

2. **The existing resource contract gives mutations the wrong lifecycle.** LLP 1005 and `runner.rs` say resources are memoized by arguments, automatically requested when those arguments change, and carried across reload when arguments match. Those are useful query semantics and dangerous effect semantics. I rank **B second**, because explicit `refresh` at least resembles a user gesture. Its expected failure is an accidental duplicate or unsolicited POST: changing/restoring the checkout pack or token already changes the resource arguments, while `refresh` also requests it. Preventing that requires coalescing rules and sentinel state; disabling automatic argument-driven requests makes it A under a misleading name. On the web, cache invalidation normally happens *after* a mutation; it is not the mutation.

3. **A preserves the intended data boundary.** Under 1016 D1–D4 and ibex LLP 0068, the Rust data crate should construct and parse the request while the browser or `ibex2::host` merely transports bytes. A does exactly that and gives each reply its own later transaction. C instead makes a host command name an application data operation and makes its typed reply part of the host/event vocabulary. Login needs no continuation: settling `session` changes the view and requests `me(session.token)`. Checkout can first expose a normal link; a host navigation command remains separate from the network mutation.

## Answers to §4 questions 2, 3, 4

### 2. Pending in the type

Keep pending **out of the value type** and retain the last value, as D3 proposes. With an authored boot value, the resource always has a valid `T`; `pending(x)` corresponds to the separate `isPending`/`isFetching` status exposed by mainstream data libraries. `option<T>` should mean domain absence, not transport state. The stale-login-error case is handled explicitly and legibly in the view: test `pending(session)` before displaying `session.error`. Making every downstream `station.name` or `session.token` unwrap a remote-state union would charge every read site for one boundary concern.

### 3. A boot value in the language

Add `else <expr>` now. Its semantics should be exact: bake type-checks and embeds the expression without invoking the remote source; the first runtime frame uses it, but it is **not** entered in the argument cache as a settled answer, so the host still requests the resource with those same arguments immediately after the first commit. For example:

```contract
resource decks = decks(session.token) as shape list<Deck> else []
```

The current “special boot arguments return `Now`” convention works for `attempt == 0`, but it conflates an idle mutation with a query answer. For an inherently remote decks list, the baked empty result would otherwise match the runtime arguments and LLP 1005’s cache rule would prevent any request until those arguments changed.

### 4. Web contradictions in D1–D6

The central D1/D2/D4 split matches `fetch`, but several claims need narrowing. Browser `fetch` has no general application-level timeout, so D4 cannot promise executor timeouts unless timeout/abort policy is added; ordinary fetch rejection should simply become `Failed`. D2 must preserve the accessible response headers and failure kind promised by D1—“status 0 plus a message” is an internal convention, not Fetch’s response model. D3’s stale-while-revalidate claim is not a browser default: `fetch` has no retained application value, and React Query/SWR make keeping previous data across a changed key configurable. Exact may choose it, but should call it Exact policy. D5’s take-latest rule is suitable for a displayed query, never for the server effect itself: forgetting a ticket does not undo a POST. Finally, D6’s grant check is an additional Exact capability boundary, not equivalent to CORS; some cross-origin requests can be sent even when their responses cannot be read.

## What I would need to see to change my mind

I would move toward B only if the real mutations proved replay-safe, idempotent desired-state writes whose cached results and automatic argument-driven execution were genuinely useful. I would add a narrow `then action(reply)` continuation to A if Weird Castle requires one-tap checkout and the intermediate “Continue to Stripe” link is unacceptable. I would choose C only if the same typed effect and reply contract truly belongs to every host rather than to the app’s Rust data source.


## Round 2 — response to the other panelist (received 2026-08-30T12:32:01Z, verbatim; run 2026-08-30T12:30:16Z → 2026-08-30T12:32:01Z; saw grok's round 1 verbatim; run log confirms `gpt-5.6-sol`, `ultra`)

## 1. Where you agree

Position unchanged: **A for mutations, with `refresh` useful as a separate query-revalidation statement.**

The other panelist is right that submitted fields are a genuine request snapshot, while `attempt` is an author-visible cache buster that should disappear. I also agree that Stripe checkout should initially settle to a URL rendered as a normal link; neither login nor checkout therefore requires a completion callback or payload branching.

Most importantly, `bricks(session.token)` remains the same query key after payment. That makes `refresh bricks` materially useful and clarifies my position: Exact needs explicit query invalidation, but that does not make the preceding POST a query resource. Pending should remain separate from `T`, commands should remain host capabilities, forgotten tickets should not imply that a POST was aborted, and grants supplement rather than replace CORS.

## 2. Where you disagree

The proposed B contract retains the precise hazard it claims to remove. Under LLP 1005, changing `submittedWho` requests `session`; the same action then executes `refresh session`. Either that produces two eligible requests, or B requires an unstated coalescing rule between argument invalidation and explicit refresh. More seriously, `checkout(session.token, packId)` remains automatically executable: if `session.token` changes while `packId` retains a previous purchase, the runner requests another checkout without `buy` occurring. Clearing every effect argument on logout merely replaces `attempt` with sentinel-state discipline.

The cited web libraries distinguish the two operations rather than unifying them. React Query’s mutation invocation creates the checkout session; query invalidation subsequently refreshes the brick balance. An HTML form similarly snapshots controls when submitted, but the resulting POST is an occurrence, not a reactive value that fires whenever the snapshot changes. Thus the other panelist’s strongest web analogy supports A for the POST plus `refresh` for `bricks`.

A also does not require `then` or action branching. LLP 1006 §1 prevents actions from branching on reply payloads; it does not prevent an effect result cell from settling and the view reacting to it. The other panelist already relies on exactly that view routing for `session.ok` and `checkout.url`, so the alleged callback cost is absent.

`checkoutReturned` is also not complete Contract semantics. A focus event, return-URL load, and “I’ve paid” button are different triggers, while 1006 only supplies actions once something invokes them. The need to refresh bricks is real; the shown declaration does not establish when it runs.

I still disagree on omitting `else <expr>`. Given D3’s total `T` plus asynchronous initial settlement, some value must exist before a no-argument remote resource resolves. Requiring its Rust source to fabricate a synchronous answer moves a presentation default across 1004 D4’s data seam. Mainstream web-query APIs instead represent absent initial data explicitly; because Exact rejects that in `T`, the Contract needs the corresponding boot value.

Finally, globally applying `cache: "no-store"` would contradict the web-default rule: ordinary `fetch` participates in HTTP caching. A forced `refresh` may require request-specific revalidation, but ordinary resource requests should retain browser cache semantics.

## 3. The strongest thing the other side said

The strongest point is that post-Stripe `bricks(session.token)` has unchanged arguments and therefore cannot become fresh under LLP 1005. It confirms that explicit query revalidation is necessary. If we are wrong about that, a successful payer can continue seeing the old balance, plausibly retry the purchase, and authors will spread generation counters through otherwise ordinary queries.

## 4. Final answers

**Q1:** Model login and checkout creation as `effect` cells invoked by `send`, and define `refresh` only for revalidating query resources such as `bricks` after the Stripe return.

**Q2:** Unchanged: pending remains separate from `T`, with the last settled value retained.

**Q3:** Unchanged: add `else <expr>` as a non-cached boot value.

**Q4:** My prior answers stand, with one addition: forced refresh must define request-specific interaction with the HTTP cache, while globally disabling caching would itself depart from browser behavior.


---

## Disposition (the orchestrator, 2026-08-30)

Folded into LLP 1016 r2 §4 "The panel" and the r2 amendments to D3–D6 (Exact policy, not a browser default; no executor timeout promised; a forgotten ticket undoes no POST; grants beside CORS; the cache rule). The recommendation to Charlie is in §4; no verdict binds (LLP 1016 §6).

## Round 3 — response to the third panelist (received 2026-08-30T12:46:34Z, verbatim; run 2026-08-30T12:44:51Z → 2026-08-30T12:46:34Z; saw Fable's round 1 verbatim; run log confirms `gpt-5.6-sol`, `ultra`)

position unchanged: A

**Where the third panelist is right**

Fable’s best contribution is making A operationally precise: a mutation is an action-invoked reply cell; `send` snapshots arguments, always creates a new request occurrence, never runs at bake, and never replays merely because a reload invalidated the carried result. Accepting only the newest reply must not imply that superseded server effects were undone.

The checkout example also cleanly separates three operations the web separates: create the Stripe session with `send`, navigate through a user-activated link, and revalidate the unchanged `balance(token)` query with `refresh`. The popup-blocker rationale strengthens the case that no completion callback is yet needed.

Two observations were genuinely new and useful: `boot_carrying` re-querying after a shape mismatch identifies a concrete accidental-POST path for resource-shaped mutations, and orphaned tickets should not keep `clock settle` waiting. The forbidden-header difference between browser Fetch and `ibex2` is also a real portability issue worth recording.

**Where it is wrong**

Making every declared `mutation ... as shape T` implicitly produce `option<T>` is not entailed by the web examples. JavaScript libraries can expose initially undefined mutation data without requiring Exact to inject transport lifecycle into the authored value type. It also understates its own sentinel cost: deriving `token = ""` immediately causes `me("")`, while the purchase example expressly requires `balance("")` to return `Now(0)`. The mutation counter disappears, but idle conventions have not disappeared from the crate.

The `logout` action is semantically unresolved under D5:

```contract
send session = logout(token)
session = none
```

If ordinary assignment does not supersede the new ticket, a late logout reply repopulates `session`; if it does, that is an additional cancellation/forgetting rule not supplied by D5, which specifies newest-request settlement rather than assignment after `send`. “One request per slot” is also imprecise: multiple POSTs may execute; only result acceptance is take-latest.

Rejecting a language boot value does not solve D3 for genuinely remote boot-time queries. A total `T` must exist before settlement. Returning a fabricated empty `Now` from the Rust source crosses 1004 D4’s data seam, and caching it under the real arguments prevents the desired runtime request. The suggested “mount task” is not available under 1006’s action model, and `refresh` cannot be invoked by an event the language does not define. Missing record/list literals constrain the spelling of `else <expr>`; they do not remove the semantic requirement.

The Fetch details also need correction. A CORS failure normally rejects `fetch`; JavaScript does not receive a response whose status is zero. Status zero characterizes opaque responses, while mapping failures to zero would be Exact glue policy. Likewise, `AbortController` is not “free” cancellation: aborting observation or transport cannot guarantee that an uploaded POST did not mutate the server. D5’s “forget, does not abort” is therefore the safer semantic guarantee, though an executor may opportunistically abort and the runner should stop awaiting the forgotten ticket.

Finally, Fetch itself has no query/mutation type distinction; React Query, SWR, and Remix do. Those libraries strongly support A, but “the web” and “every mainstream library” overstate the evidence.

**My final one-sentence answer to §4 question 1**

Model login and checkout creation as typed result cells invoked only by `send`, and reserve `refresh` for revalidating queries such as the post-Stripe balance.

Q2 and Q3 are unchanged: pending stays separate from `T`, the last settled value is retained, and `else <expr>` supplies a non-cached boot value. Q4 gains the forbidden-header portability issue and the requirement that forgotten tickets not block settlement, while retaining the prior CORS, timeout, grant, and HTTP-cache qualifications.


---

## Disposition, final (the orchestrator, 2026-08-30)

Three panelists, three rounds in all (codex and grok: blind, each other, then the third panelist; Fable: blind, then both). Folded into LLP 1016 r3 §4 "The panel" — the tallies, the one live disagreement (A with `mutation`/`send` and `refresh`, two panelists; B with a method-aware D5, one), the narrowings all three accepted, and the recommendation to Charlie. No verdict binds (LLP 1016 §6).
