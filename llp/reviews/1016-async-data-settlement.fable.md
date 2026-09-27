# Panel: LLP 1016 Asynchronous data settlement — §4, how a mutation is written (fable)

- **Family:** Claude (Anthropic): Claude Fable 5 (`claude-fable-5`) as a fresh subagent through Claude Code's Agent tool — a new context with no view of the orchestrating session, instructed to read only the panel directory's brief and capsule (the same 16 files the other panelists had) and nothing under the repositories; reasoning effort is the harness's default for a subagent (not pinnable from the tool; Charlie asked for xhigh or max). Joined at Charlie's request after the other two panelists' rounds 1–2 had run; its round 1 was blind to them.
- **Method:** the panel of LLP 1016 §6. Brief (round 1, third-panelist wording) sha256 0d6daa70cb6495be…; LLP 1016 (r1, in the capsule) sha256 fa8ed97e6c088c2d26d31d1f294fd821e4bd9c62313f2a9d2ce8e6da9f2873f3; capsule sha256 9af90cce34517544ec3907f13ab189e7112e3a2ae715084f4322f2c61ff96861.
- **Disposition:** the fold is recorded in LLP 1016 §4 and at the end of this file.

---

## Round 1 — position (received 2026-08-30T12:44:46Z, verbatim; blind to the other panelists)

# LLP 1016 panel, round 1 — Fable

## 1. Position

**A, made precise: a `mutation` is a typed reply slot, and `send` is the statement that fills it later.** Queries stay exactly as the RFC has them (D1–D6). A mutation is not a resource and needs no counter: it is a slot of type `option<T>`, `none` at boot, which an action fills with `send name = source(args)`. `send` calls the same `DataSource::query`: `Now` writes the slot inside the action's commit (so the stand-in keeps working unchanged), `Later` leaves the slot pending with a ticket, and fulfilment is D5's settlement pass — `parse`, write the slot, derives and resources that read it follow. No callback, no `then`: the view branches on the reply, resources chain off it. One request per slot, newest wins (D5). Nothing runs at bake; a reload carries the slot's value like any slot and never re-sends; the arguments are taken at the `send`, as a form takes its fields at submit.

Login:

```
shape Session
  ok: bool
  username: string
  token: string
  error: string

component WeirdCastle
  state screen = "title"
  state who = ""
  state password = ""
  mutation session as shape Session        // option<Session>: none until a reply lands

  derive signedIn = match session { case some(s) => s.ok, case none => false }
  derive loginError = match session { case some(s) => s.error, case none => "" }
  derive token = match session { case some(s) => s.token, case none => "" }
  resource me = me(token) as shape Profile  // follows a login by itself (D5); shape elided

  action submit writes session
    send session = login(who, password)
  action logout writes session, screen, who, password
    send session = logout(token)           // forgets a login still in flight (D5)
    session = none                         // the surface drops now, not when Castle answers
    screen = "title"
    who = ""
    password = ""

  view
    main ...
      when signedIn
        SignedIn(...)
      else
        ...Login(error=loginError, busy=pending(session), ...)
```

Brick purchase (LLP 0004: checkout on the web, the balance in Castle):

```
shape Checkout
  ok: bool
  url: string
  error: string
shape Balance
  bricks: number

  resource balance = balance(token) as shape Balance   // Now(0) for "", Later once logged in
  mutation checkout as shape Checkout

  action buy(quantity) writes checkout
    send checkout = createCheckout(token, quantity)
  action paid writes balance
    refresh balance                                     // B's statement, for a query — §2

  view
    text `${balance.bricks} bricks`
    match checkout
      case some(c) =>
        when c.ok
          link href=c.url label="Continue to checkout"  // the user taps; the popup blocker insists
        else
          text c.error
      case none =>
        button press=buy(100) "100 bricks"
        button press=buy(500) "500 bricks"
    when pending(checkout)
      text "Preparing checkout…"
    button press=paid "I'm back — refresh my balance"
```

The purchase needs one more statement, `refresh <resource>`, and needs it for a *query*: the balance changed on Stripe's page, outside the app, and no argument of `balance` changed. That is B's statement in its right place; it lands with the purchase, not with the login.

## 2. Why

**1. The web separates queries from mutations, and so does every mainstream library.** A form POSTs on submit, never because a field changed; `fetch` is called; React Query has `useQuery` (keyed, cached, refetched on key change) and `useMutation` (`mutate(variables)`, unkeyed, never at mount); SWR has `useSWR` and `useSWRMutation`'s `trigger()`; Remix has loaders and `<Form method="post">`/`fetcher.submit`. None models a mutation as a query keyed by its inputs, for the reason the RFC's own counter exposes: a query with equal inputs must not run twice, a mutation must. P files unsafe requests in the safe table, so every re-request rule the runner ever gains — D5's reload, revalidate-on-focus, retry — has to be audited against "would this charge a card".

**2. A mutation modeled as a derivation of state gets re-derived, and the platform says no.** Today `boot_carrying` re-queries any resource whose carried value no longer fits its shape (`runner.rs`: `settle(false)` after the carry), and 1016 D5 says a reload "re-requests what has no compiled value". Under P, editing `shape Session` while building the login screen re-sends the credentials on the next save; editing `shape Checkout` creates another Stripe session. The browser encodes the opposite rule in "Confirm Form Resubmission" and the POST-redirect-GET pattern: a POST is not regenerated from state. P's deeper wart is not the counter but that the value is a function of its arguments, so `submittedPassword` must stay in a slot for the life of the session — clearing it changes the arguments and logs the user out — visible in `state`, carried across reloads. A form's fields can be cleared after submit because the submission owns its body; `send` gives Contract that.

**3. No continuation, and the web says why.** The RFC fears A needs `… then onLogin`. It does not: the reply is data, the view branches, resources chain (D5's `me(session.token)`). The one place a continuation looks needed — opening the checkout URL when it arrives — is precisely what Safari's popup blocker refuses (`window.open` outside a user gesture); the web's answer is a link the user taps. Fulfilment runs no action, as D5 says, and 1006 §1 stands: actions assign and command, never branch.

**Second: B.** Its failure: a resource with a side effect fires when its inputs change, not when the user acts. `resource checkout = createCheckout(token, quantity)` creates a session when a pack is picked and again when a re-login changes the token; to stop that the author reintroduces the submitted-copy slots, so `refresh` removes the counter and leaves the root. Keep `refresh` for what it is on the web (`mutate(key)`, `invalidateQueries`, Remix's revalidation): re-request a query whose inputs did not change.

Cost, honestly: one declaration (`mutation` — a slot row flagged sendable, type `option<T>`, an assignment is allowed), one statement (`send` — one opcode beside `Command`), `writes` may name a mutation, a ticket per mutation slot sharing D2/D5's `fulfill`. Against it: no counter, no submitted copies, no `attempt = 0` convention per source in the crate, nothing queried at bake for mutations. If the language ever gains typed `state` declarations, `mutation` collapses into `state session: option<Session> = none`.

## 3. Answers

**Q2 — D3 holds for queries; a mutation's reply is `option<T>`.** Stale-while-revalidate is the web's rule for reads: a navigation keeps the old document until the new one arrives, SWR is named for it, React Query calls it `placeholderData`; `pending(x)` is `isFetching`. Retyping `station.name` for a source that might become remote is the wrong trade, as the RFC says. But React Query's `useMutation` starts with `data: undefined`; "nothing sent yet" is genuinely `none`, not a stale idle record; and the slot is new, so nothing is retyped. During a retry keep the last reply and let `when pending(session)` blank the stale error — React Query resets on a new `mutate`, Remix keeps `fetcher.data`; keeping is one view line to undo, clearing is not recoverable.

**Q3 — No boot value in the language now.** For mutations `none` is the boot value and the question disappears. For queries, "boot arguments the source answers `Now`" is bake as it exists, and anything behind login (`decks(token)`, `balance(token)`) requests itself when the token arrives. `as shape T else <expr>` also smuggles in a second addition: Contract has no record or list literal to write the `<expr>` with (1006 §2). The day a resource is remote at boot and behind nothing, the web's answer is the empty shape plus `pending`, with `refresh` in a mount task as the trigger — the same `refresh` the purchase needs.

**Q4 — What the web contradicts.** (i) D5's reload re-request, if mutations are resources: the browser never silently re-POSTs. Resolved by A. (ii) "Timeouts are the executor's": `fetch` has none, so on the web a request can pend indefinitely unless the glue passes `AbortSignal.timeout`; and "forgets, does not abort" should read "forgets; the web glue aborts" — `AbortController` is free, and `clock settle` must not wait on a ticket nobody holds. (iii) Forbidden request headers (`Cookie`, `Host`, `Origin`) are dropped silently by `fetch` and sent by ibex2 — a parity hazard the data crate should know. (iv) A CORS refusal arrives as status 0 like a dead network; only the glue's own grant check yields `Refused` — fine, but say so. (v) Not a contradiction, a difference: the one-deep memo means a search that returns to earlier text re-requests, where React Query and SWR serve the keyed cache. Failure as data (`fetch` resolves on 4xx), non-2xx as a response, the request/response pair, and grants all match.

## 4. What would change my mind

Show me the purchase written as P or B that (a) does not keep `token` and `quantity` in slots for the life of the checkout, (b) is not re-sent by `boot_carrying` or D5's reload when `shape Checkout` gains a field, and (c) needs no `attempt = 0` idle convention per source in the crate — in fewer lines than `mutation` + `send`. Or: if the purchase is outside v1 and the login is the only mutation Weird Castle ships, B is acceptable for the login alone — a re-sent login is harmless — provided the first purchase is not written that way.


## Round 2 — response to both panelists (received 2026-08-30T12:49:24Z, verbatim; saw codex's and grok's rounds 1–2 verbatim)

position unchanged: A — `mutation name as shape T` (an `option<T>` slot, `none` at boot) filled by `send name = source(args)` in an action, plus `refresh <resource>` for queries.

## 2. Where I agree

**With both.** `refresh <resource>` is needed and it is a query verb: after Stripe, `balance(token)` has the same arguments and must be asked again, which 1005 §5 will not do. D3 holds for resources; pending is `pending(x)`, not the type. No `then`, no callback: the view routes on the reply. C is out (1004 D4). The Stripe URL is a link the user taps. D4 cannot promise a timeout `fetch` does not have; D6 is a grant, not CORS; a forgotten ticket never undid a POST.

**With codex.** 1005's lifecycle — memoize by arguments, request on change, carry on reload — is a GET lifecycle. `checkoutReturned` names a trigger Exact does not have: events are press/change/hover/focus/blur/key on views (1005 §6) and `every(ms)` tasks; a return-from-checkout event is a later host ask, and until then the balance refreshes from a button or a mount timer. On caching codex is right and I was loose: the browser's `fetch` uses the HTTP cache by default and Exact must not globally `no-store`; the data crate puts `Cache-Control: no-cache` on the balance request, as a web app does, and the cache never serves a POST anyway.

**With grok.** Coalescing costs nothing: `settle` runs once per action (runner.rs), so an argument change and a `refresh` in the same action are one query. Under B the checkout must snapshot the token — grok's correction is the right one for B. No `else <expr>` now: a placeholder that is then fetched is a third lifecycle against D3's "the baked value is the settled answer", and 1006 §2 has no list or record literal to write `else []` with; codex's `idleSession` resource is a second copy of the crate's idle record.

## 3. Where I disagree

**Grok: "1005 carries a settled resource and does not re-request."** Only while its value still fits. `boot_carrying` keeps a resource where `check_shape` passes and then `settle(false)` queries the source with the carried arguments (runner.rs). So under B, editing `shape Session` while signed in — the ordinary dev-loop moment (1007 §6) — re-POSTs the credentials on every save; editing `shape Checkout` creates a Stripe session per save. Snapshotting cannot fix that; only not being a resource can. Under A the carried slot no longer conforms, resets to `none`, and nothing is sent: signed out is also the browser's failure ("Confirm Form Resubmission").

**Grok: "GET and POST are one mechanism."** `fetch` is one function; the platform splits by method everywhere it matters: the HTTP cache serves GETs and not POSTs, a GET form re-runs on reload and a POST form asks first, RFC 9110 names safe and idempotent methods. Every re-request rule the runner has — arguments changed, carry, D5's newest-wins — is a GET rule, and under B the runner cannot see the method: D1 builds the `Request` inside `query`, after the runner has already decided to send. The one fact the runner needs and cannot infer is which kind this is; `mutation` is that fact.

**Grok: the author reaches for B without a spec.** B needs four disciplines: copy the live fields (or keystrokes send); snapshot the token (or a re-login re-POSTs); never clear the submitted slots after success (the arguments would change and log the user out — so the password stays in `state`, visible in the agent's `state` and carried across reloads); never edit the shape while signed in. `send session = login(who, password)` makes all four impossible. And the A grok argues against is codex's — `effect`, `else`, idle stubs. Mine has none: `none` is the boot value.

**Codex: the cell is `T` with an idle boot.** That is what forces `else` or an idle resource, and codex concedes that fabricating the idle answer in Rust moves a presentation default across 1004 D4's seam. `option<T>` is what `useMutation` has (`data: undefined` until a reply); "nothing sent yet" is absence, not transport state — pending stays `pending(x)`. The retyping cost is three derives on a new slot, not `station.name`. Keep `else <expr>` out until a bake fails for a no-argument remote resource.

**Codex: newest-wins is wrong for the effect.** For what is *shown* it is right and cheap; both POSTs were sent, as two `fetch`es are, and `pending(checkout)` disables the button. One ticket per slot stays.

## 4. The strongest thing either said

Grok's: DEFERRED binds one authoring model, and A is a second way to fill a value. If I am wrong, Exact carries a `mutation` row and a `send` opcode that `refresh` plus discipline could have covered, and every app has two kinds of remote value. If grok is wrong, the first ledger debit in LLP 0004 — "generate my avatar", a fixed brick price against a signed receipt — is spelled as a resource and re-runs on a shape edit, a carry, or a token change; and passwords live in state. A's mistake costs a table row; B's costs a charge. The asymmetry decides it.

## 5. Final

**Q1.** Write a mutation as `mutation name as shape T` (`option<T>`, `none` at boot) filled by `send name = source(args)` from an action, add `refresh <resource>` for a query whose arguments did not change (the balance after Stripe), and never spell a POST as a resource — every re-request rule the runner has is a GET rule and the runner cannot see the method.

**Q2.** Unchanged: D3 holds for resources; a mutation's reply is `option<T>`.

**Q3.** Unchanged: no `else <expr>`; `none` answers it for mutations and bake answers it for queries.

**Q4.** One change: no global `no-store` — the browser's cache default stands and the data crate marks mutation-adjacent GETs `Cache-Control: no-cache`. The rest stands: no promised timeout; `clock settle` never waits on a forgotten ticket; grants supplement CORS; a carried reload must never re-send.


---

## Disposition, final (the orchestrator, 2026-08-30)

Three panelists, three rounds in all (codex and grok: blind, each other, then the third panelist; Fable: blind, then both). Folded into LLP 1016 r3 §4 "The panel" — the tallies, the one live disagreement (A with `mutation`/`send` and `refresh`, two panelists; B with a method-aware D5, one), the narrowings all three accepted, and the recommendation to Charlie. No verdict binds (LLP 1016 §6).
