# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 3, second narrow re-check (the Proxy tracking) (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `64bdb5a10e`.
- **Method:** one brief (sha256 `aab0c3ebe476b4081f065ad373ddbd67b6af27ac70eae6e7a566701fea528bac`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `js/src/binary.js:457` — On this Hermes, a callable proxy's `[[Construct]]` runs `ordinaryCreateFromConstructor` on `new.target` before it calls the `construct` trap (`JSCallableProxy::_proxyNativeCall`). That gets `new.target.prototype`. The engine's `Proxy` constructor does not: it only checks that it was called with `new` and creates the proxy. `new Proxy(...)` still matches the engine, because `new.target` is the wrapper and that get forwards to the engine `Proxy`, which has no `prototype`, so no user code runs. `Reflect.construct(Proxy, args, Ctor)` does not: a getter, a proxy, or a throwing `Ctor.prototype` runs or aborts the call, and the engine call would have succeeded. The `target === tracked ? fn : target` choice only affects the inner `Reflect.construct`; the outer get has already happened. The fixture's `Reflect.construct(Proxy, [{ c: 3 }, {}], Object)` stays green because `Object.prototype` is a data property. `new (Proxy.bind(...))(...)` has the same extra get on the bound function. Fix: in `JSCallableProxy::_proxyNativeCall`, skip `ordinaryCreateFromConstructor` when a `construct` trap is present and return the trap's object, which is what the spec requires. Until that engine change, also replace `Reflect.construct` (same descriptor, a proxy over the engine function) so that when the target is the wrapper it calls the engine `Proxy` directly, records that result, and forwards every other target — that covers the explicit-`new.target` path, not `new` on a bound `Proxy`.

**Non-blocking**

none
