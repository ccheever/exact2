# Panel: LLP 1016 Asynchronous data settlement — §4, how a mutation is written (grok)

- **Family:** grok (xAI): `grok -p <prompt> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`; the prompt embedded the brief and every capsule file with a full terminal ban; workdir = the capsule (16 files: LLP 1016, 1004, 1005, 1006, 1007, 1012, `rules/RULES.md`, `rules/DEFERRED.md`, `runner/src/runner.rs`, the caltrain and weird-castle contracts, weird-castle `data/src/lib.rs`, weird-castle `llp/0000`, `0004`, ibex LLP 0067, 0068), no network.
- **Method:** a panel at Charlie's request, 2026-08-30 (LLP 1016 §6: not a refine loop; no verdict binds). Round 1 mutually blind; round 2 each panelist sees the other's round-1 position verbatim. Brief (round 1) sha256 57b579c1f9fbfa777c497943d6e7f59f7e4593bd54e3a42e7389ba9885a53895; LLP 1016 sha256 fa8ed97e6c088c2d26d31d1f294fd821e4bd9c62313f2a9d2ce8e6da9f2873f3; capsule sha256 9af90cce34517544ec3907f13ab189e7112e3a2ae715084f4322f2c61ff96861.
- **Disposition:** the fold is recorded in LLP 1016 §4 and at the end of this file.

---

## Round 1 — position (received 2026-08-30T12:25:43Z, verbatim; run 2026-08-30T12:20:07Z → 2026-08-30T12:25:43Z)

> Provenance note: the CLI's three leading status lines ("I'll read…") are its own narration while it read the embedded capsule; preserved as received.

I'll read the full offloaded prompt as text so I have the complete LLP 1016 capsule and the questions before taking a position.The prompt is large; next I’ll pull LLP 1016, the runner/language notes, and the two real contracts so the position is grounded in those texts.I have the RFC and login contract; next I’ll pull the Stripe checkout details and how settlement actually re-requests, since those decide P vs B.# 1. Position

**B** — keep P’s shape (a resource is a function of state) and add one action statement, `refresh <resource>`, which re-requests that resource with its current arguments even if they did not change. The submitted snapshot is real (it is the form’s request body); the retry counter is not. Login copies the live fields and refreshes `session`. Buying bricks copies the pack, refreshes `checkout` (a POST that returns a Stripe URL), and on return refreshes `bricks`. The URL is a `link` in the view: fulfilment runs no action (1016 D5), and today’s actions cannot branch on a payload (1006 §1). No new table, no reply-event, no `then`.

Login:

```
shape Session
  ok: bool
  username: string
  token: string
  error: string

state screen = "title"
state who = ""
state password = ""
state submittedWho = ""
state submittedPassword = ""

resource session = login(submittedWho, submittedPassword) as shape Session

action submit writes submittedWho, submittedPassword
  submittedWho = who
  submittedPassword = password
  refresh session

action logout writes screen, who, password, submittedWho, submittedPassword
  screen = "title"
  who = ""
  password = ""
  submittedWho = ""
  submittedPassword = ""
```

Logout needs no `refresh`: empty arguments are a new key and the source answers `Now` (idle), as bake already does. Empty-field submit and same-password retry do need it — those keys already settled.

Brick purchase (LLP 0004: link out to Stripe, ledger on Castle):

```
shape Checkout
  url: string
  error: string

shape Bricks
  balance: number
  error: string

state packId = ""

resource checkout = checkout(session.token, packId) as shape Checkout
resource bricks = bricks(session.token) as shape Bricks

action buy(pack) writes packId
  packId = pack
  refresh checkout

action checkoutReturned
  refresh bricks
```

View: `when pending(checkout)` / `when checkout.url != ""` then `link checkout.url` (Pay with Stripe), else the Buy button. Same `pending(session)` on Log in. Two-step pay is the web’s Payment Link; it is not a `then` callback.

# 2. Why

**The model is already state → resources → view.** LLP 1004 D4 and 1006 §1 make a resource `source(args)` over state; 1005 §5 re-requests only when arguments change. Caltrain is that rule (`search(query, location)` on each keystroke). Login must not be: the live fields are the inputs, the submitted fields are the POST body — the same split an HTML form makes at submit. P already writes that. What P then adds, `attempt`, is a cache-buster for a memoized GET. The browser does not memoize POST; SWR/React Query do not ask the author for `attempt`; they ask for `mutate()` / `invalidateQueries` on the same key. `refresh` is that verb, in a language that already has assignment and command as the only statements.

**B is the one addition that pays, and it pays on the second mutation.** Login can be beaten into working with `attempt` (Weird Castle already did). After Stripe, `bricks(session.token)` is the same key it was before the user paid. P will not re-request. The author then invents `bricksGen` and starts threading generation counters through queries that are not submissions. That is a second, unofficial language. `refresh bricks` in `checkoutReturned` (focus, return URL, or an “I’ve paid” press) is the sentence they mean. DEFERRED: a new table, a callback, or a host-owned contract has to pay against the author’s understanding. B is one statement and a bit on the action; A and C each open a second way to fill a value.

**The web’s executor is `fetch`, not Elm and not a command bus.** 1016 D1–D2 already match `fetch` plus a parse step. Commands today are host capabilities (`setScheme`, 1005 §3, 1007 §4), not GraphQL. Putting `loginV2` or Stripe session creation in C moves the effect’s contract into the host and contradicts 1004 D4 (data crate owns the seam). A is Elm `Cmd`/`Msg` in a language whose actions cannot `if` on a payload (1006 §1). Without `then onLogin`, A still adds a plan table and a second pending/fill path; with `then`, it asks for the branching the language refused. The author already routes on `session.ok` in the view. Keep it there.

**Second place is P.** Failure: same-key retry and post-Stripe `bricks` silently keep the last value. The login form looks dead; the balance looks unpaid. People will copy `attempt` onto every resource, including ones that should stay a query, and they will get the boot-sentinel mix-up (`0` means idle and also “never retry”) wrong on the third mutation.

# 3. Answers to §4 questions 2, 3, 4

**Q2 — D3 holds; pending is not in the type.** The browser keeps showing the previous document until the next one arrives; SWR keeps `data` and sets `isValidating`; React Query’s `isFetching` is a parallel flag. That is `pending(resource) → bool` in expressions (`when pending(session)`), which 1016 already adds. Making every remote resource `option<T>` retypes Caltrain’s `station.name` and every later read site, and it still cannot tell “not yet” from “none” without a second signal. Stale error during a login retry is real: hide it with `when pending(session)`, or keep showing it as SWR does. Do not spend the retyping cost to avoid one `when`. When arguments change, keeping the previous value is HTML’s old page and Caltrain’s old board; for a token change it can flash the wrong user — the view already has `session.ok` / `pending(me)` to avoid rendering `me` until the new key settles.

**Q3 — no `else <expr>` now.** Bake stays synchronous and a `Later` at bake is a build error (1016 D3). Login’s idle is empty arguments the source answers `Now`; that is enough. `me` and `decks` after login are not bake: their baked value is the empty-token one, and D3 keeps it while the first real request is out. A language boot value is a third copy of what the data crate already returns for the initial args. Add `as shape T else <expr>` when some resource’s *initial* arguments cannot be answered `Now` (a no-arg `flags()` that must hit the network). Until a bake actually fails that way, the `else` is speculative machinery.

**Q4 — two real mismatches, neither fatal to D1–D6.** (1) Glue `fetch` will honour the HTTP cache by default. `refresh bricks` after Stripe can then replay a cached GET and look like P’s memoization bug. The web’s own apps mark mutation-adjacent GETs `cache: 'no-store'` (or `reload`). Do that in `glue.js` for `request` ops; POST for `login`/`checkout` in the data crate, which D1 already allows. (2) D5 forgets a ticket and does not abort. That matches `fetch` without `AbortController`, not a document navigation (which aborts the previous load). Leave it: abort would not stop Stripe or `loginV2` from committing, and 1016 §5 already leaves cancellation off the RFC. Do not treat grants as replacing CORS; D2 is right that CORS remains the web authority and D6 is an extra identical refusal. `status 0` for a thrown `fetch` is glue, not a model break. Auto-redirect to Stripe on fulfil would be a completion callback, which D5 forbids and which HTML does with a link or a 303, not with a client `then`.

# 4. What you would need to see to change your mind

I would take **P** if the next two mutations after login each naturally change arguments (no same-key retry, no “same token, new GET”), so `refresh` would sit unused while `attempt` stays local. I would take **A** if Contract gains a payload-free completion hook that only assigns (still no `if`), *and* Stripe auto-navigation is a product requirement a `link` cannot meet — and even then only for host navigation, with the POST still a resource. I would take **C** only if the effect’s contract cannot live in the data crate; 1004 D4 says it must. I would put pending in the type if a single identity mix-up (wrong `me` while the new token is in flight) showed up in the agent drive and `pending`/`when` could not close it. I would add `else` the first time bake names a resource that cannot answer `Now` for its initial arguments.


## Round 2 — response to the other panelist (received 2026-08-30T12:33:33Z, verbatim; run 2026-08-30T12:30:18Z → 2026-08-30T12:33:33Z; saw codex's round 1 verbatim)

> Provenance note: the CLI offloads a long prompt to a file and its first line says it re-read the offloaded prompt to recover the other position in full; preserved as received.

The other panelist’s round-1 text was truncated in the brief, so I’ll read the offloaded prompt as text to recover the full position before answering.# 1. Where you agree

**Position unchanged: B.**

We agree on the product shape and on most of §4. Login and checkout-session creation are occurrences: two submits are two `fetch`es; `attempt` is not application state. Queries may stay argument-keyed under LLP 1005 §5; a POST must not fire merely because its arguments exist. React Query’s `mutate` / SWR’s `trigger` are that occurrence. C is out: LLP 1004 D4 keeps the seam in the data crate, and today’s commands are host capabilities (1005 §3), not typed app operations. The Stripe URL is a view `link`; fulfilment runs no action (1016 D5); no `then`. Pending stays out of the value type (browser previous document; SWR/React Query keep `data` and a parallel flag). Stale `session.error` is a `when pending(session)` in the view. Forgetting a ticket does not undo a POST. D6 grants are not CORS. Auto-redirect is not required.

Their unsolicited-POST failure of naive B is real, and it does change the **checkout spelling**, not the option. Round-1 `checkout(session.token, packId)` binds a live token, so a later token change re-POSTs under 1005. That is not how an HTML form works: the POST body is the values at submit. Checkout must snapshot token and pack in the action, the same way login already snapshots `who`/`password`. I also take their Q4 narrowings: D4 must not promise an executor timeout `fetch` does not have; D3’s keep-previous-value is Exact policy, not a browser default.

None of that is `effect`/`send`. Invalidation after a mutation is not the mutation — they said this, and it is why brick purchase still needs `refresh bricks` after return, which their contract does not write.

# 2. Where you disagree

**A is a second way to fill a `T`.** DEFERRED is one authoring model. Caltrain is already `resource = source(args)`. A adds `effect`, `send`, `else`, and dummy `idleSession` / `idleCheckout` resources so the cell has a `T` before anyone has sent. The author now classifies every remote value as query or mutation. `fetch` and HTML do not: GET and POST are one mechanism; the difference is method and when it runs. Mainstream libraries split `useQuery`/`useMutation`; the web platform does not, and 1016 §1 binds the platform.

**Their A does not answer brick purchase.** After Stripe, `bricks(session.token)` is the same key (1005 §5). They correctly say cache invalidation happens *after* a mutation. That verb is `refresh`, not `send`. A without it leaves the balance looking unpaid — the failure I assigned to P — unless the author invents a generation field, which they already reject as `attempt`. The web’s actual pattern is mutate, then revalidate a query. That is B on the query, not a new effect table.

**“Disabling argument-driven request makes B into A” is false if the resource args are the submit snapshot.** 1005 already re-requests only when arguments change. Login args change only in `submit`. Checkout args change only in `buy`, which copies `session.token` and `pack` into submitted cells, then `refresh checkout`. Token rotation and pack restoration do not touch those cells, so they do not POST. HTML does not re-POST a form because a cookie changed.

**Coalescing is not extra machinery.** LLP 1005 §5: settlement is transactional. In one action, argument change and `refresh` are one request, not two. `refresh` is OR, not a second `fetch`. Duplicate POST on `buy` of a new pack is a misreading of the runner you already have.

**Reload:** 1005 already carries a settled resource when arguments match and does not re-request. That is “retain the result, do not replay the POST.” A’s “not replayed after reload, but the cell may be carried” is the same fact with a new table. Production reload of an effect that is *not* carried logs the user out until they send again; a session resource of submitted credentials (or, better, `me(token)` as a query once the token is state) does not.

**`else <expr>` now, and their idle resources, fight 1016 D3.** Bake is synchronous; `Later` at bake is a build error; the baked value *is* the settled answer for those arguments. Their `else []` that is “not entered in the argument cache, then requested with the same args” is a third lifecycle (placeholder, then implicit fetch). Login idle is empty submitted args the source answers `Now`, as bake already does. `decks(session.token)` at bake is the empty-token value; after login the token changes and 1005 requests. That is not the cache-stuck case they describe. Dummy `idleSession()` resources are a second copy of what the data crate already returns.

**`send session(who, password)` reading live fields is fine as an action-time snapshot; it does not justify `effect`.** The live/submitted split is already how the login contract is written. A deletes those fields by moving the snapshot into `send`, and pays with a new declaration form.

# 3. The strongest thing the other side said

That **1005’s resource lifecycle — memoize by args, auto-request on change, carry on reload — is the wrong lifecycle for a mutation**, and that B’s expected failure is an accidental duplicate or unsolicited POST (pack/token change already requests, `refresh` requests again; turning auto-request off is A under another name).

Cost if I am wrong: two Stripe Checkout sessions for one Buy, a login POST on restore or token rotation, and any later non-idempotent mutation (ledger debit, grant) committing twice. Coalescing and submitted snapshots are easy to specify and easy to get wrong in the runner; a second `effect` table makes “this does not run unless the action says so” unmissable.

Cost if they are wrong: every app forever has two network authoring models; login grows `idle*` stubs and `else`; brick balance still needs a refresh verb they did not add; Caltrain-shaped queries stay simple only until someone puts a POST in a resource “because it returns a shape.”

# 4. Final answers

**Q1:** Charlie should add one action statement, `refresh <resource>`, keep login and checkout as resources of *submitted* snapshots (checkout must copy `session.token` and pack in `buy`, not bind the live token), coalesce refresh with argument change into a single request per transaction, and not add `effect`/`send`.

**Q2:** Unchanged, and we agree — pending is not in the type.

**Q3:** Unchanged — no `else <expr>` until bake names a resource that cannot answer `Now` for its initial arguments. Their boot-placeholder-then-fetch contradicts 1016 D3.

**Q4:** Unchanged on glue `cache: 'no-store'` for mutation-adjacent GET and on D5 not aborting. Add theirs: D4 must not promise timeouts; D3 keep-previous is Exact policy; D6 is an extra refusal, not CORS.


---

## Disposition (the orchestrator, 2026-08-30)

Folded into LLP 1016 r2 §4 "The panel" and the r2 amendments to D3–D6 (Exact policy, not a browser default; no executor timeout promised; a forgotten ticket undoes no POST; grants beside CORS; the cache rule). The recommendation to Charlie is in §4; no verdict binds (LLP 1016 §6).

## Round 3 — response to the third panelist (received 2026-08-30T12:49:24Z, verbatim; run 2026-08-30T12:44:56Z → 2026-08-30T12:49:24Z; saw Fable's round 1 verbatim)

> Provenance note: the CLI's two leading status lines are its narration while reading the offloaded prompt; preserved as received.

I'll read the full prompt and the capsule texts as files so the reply to the third panelist is grounded in those documents.The prompt is long; I'll keep reading LLP 1016, prior rounds, and the login/checkout contracts.position unchanged: B

**Where the third panelist is right**

Fable named several facts that were not in my two rounds.

Today’s `boot_carrying` then `settle(false)` (`runner.rs`) **queries** a resource whose carried value no longer conforms. Editing `shape Session` or `shape Checkout` while `submittedWho` / `submittedPassword` / pack still carry **re-POSTs**. 1016 D5’s “re-request what has no compiled value” makes that worse. The browser’s rule is the opposite: “Confirm Form Resubmission” and POST-redirect-GET.

Under P/B the POST body lives in slots for the session’s life. Clearing `submittedPassword` changes arguments and logs the user out (1005 §5). A form can clear its fields because the submission owns the body. Agent `state` also dumps the password.

1006 §2 has no record or list literal, so `as shape T else <expr>` cannot write a `Session`. Q3 is blocked by the language, not only by bake.

Safari’s popup blocker (`window.open` outside a user gesture) is why the Stripe URL is a `link` the user taps (LLP 0004: link out). Fulfilment runs no action (1016 D5). Agreed.

`fetch` has no default timeout; `Cookie` / `Host` / `Origin` are dropped by `fetch` and sent by ibex2; a CORS failure arrives as status 0. Those are Q4 glue/parity notes.

After Stripe, Castle’s balance changed and no argument of `balance(token)` did. That verb is `refresh` on a **query**. Invalidation is not the mutation.

React Query’s `useMutation` starts with `data: undefined`. “Nothing sent yet” is genuinely `none`, not an idle `Session` record.

**Where it is wrong**

**This is not RFC A.** RFC A is an effect statement (and a plan table). Fable’s purchase still has `refresh balance`. That is B, which bricks need whether or not login is a `mutation`. A without it leaves the same-key GET unpaid (1005 §5). The RFC asked P/A/B/C; the contract they wrote is A+B.

**The web’s split is method and submit, not a second declaration.** 1016 §1 binds to `fetch`; D1 already has `Request.method`. HTML POSTs on submit; it has no `useMutation`. Remix is loaders vs `<Form method="post">` — still one `fetch`. Put the replay rule on D5: **do not re-request POST** on reload, shape-miss, focus, or retry. That is PRG without a `mutation` keyword. P files POSTs in the resource table; D1’s method is the audit. DEFERRED is one authoring model.

**Submitted snapshots are the POST body**, not P’s wart. The wart is `attempt`. B copies `who`/`password` (and token + quantity in `buy`) in the action, then `refresh`. `resource checkout = createCheckout(token, quantity)` on live inputs is a misspelling of B. 1005 already re-requests only when arguments change; snapshots mean a re-login does not create another Stripe session.

**Logout contradicts D5.** `send session = logout(token)` then `session = none` on one slot: one request per slot, newest wins. If assignment forgets the ticket, Castle never sees logout. If it does not, fulfill overwrites `none`. Local clear and a logout POST are two things. Weird Castle today resets slots (`attempt = 0`); idle is `Now` in the data crate.

**`option<T>` on mutations fights D3 and the login contract.** Weird Castle reads `session.ok` / `session.error` / `session.username`. Fable adds three `match` derives. D3 rejected option so read sites keep their type. Stale error is `when pending(session)` (already in 1016). React Query is not the platform.

**Bake idle is D3, not `attempt = 0`.** `Later` at bake is a build error. B’s `checkout` with `packId = ""` must answer `Now` (empty key), as `login` already does for `attempt <= 0`. That is one `Now` in the crate, not a per-source counter. Fable’s (c) is the counter, which B already dropped.

**`send` is a second way to fill a `T`** (an opcode beside `Command`, 1005 VM). They correctly skip `then` (1006 §1). They still classify every remote value and still need `refresh`. If typed `state` arrives, collapsing `mutation` into `state session: option<Session> = none` does not remove that second fill path.

**Q1:** Charlie should add `refresh <resource>`, keep login and checkout as resources of submitted snapshots (checkout copies token and quantity in `buy`), coalesce refresh with argument change into one request, and write D5 so POST (`Request.method`, D1) is never replayed on reload, shape-miss, or a later focus/retry rule — not `mutation`/`send`.

**Q2:** Unchanged — pending is not in the type; mutation slots as `option<T>` are the retyping D3 refused.

**Q3:** Unchanged — no `else <expr>`; 1006 §2 has nothing to write the expr with.

**Q4:** Keep glue `cache: 'no-store'` for mutation-adjacent GET, and D5 forgets rather than aborts (1016 §5). Take Fable’s: D4 must not invent a `fetch` timeout; name forbidden-header drop and CORS-as-status-0 as glue/ibex2 parity; D6 `Refused` is the grant check only.


---

## Disposition, final (the orchestrator, 2026-08-30)

Three panelists, three rounds in all (codex and grok: blind, each other, then the third panelist; Fable: blind, then both). Folded into LLP 1016 r3 §4 "The panel" — the tallies, the one live disagreement (A with `mutation`/`send` and `refresh`, two panelists; B with a method-aware D5, one), the narrowings all three accepted, and the recommendation to Charlie. No verdict binds (LLP 1016 §6).
