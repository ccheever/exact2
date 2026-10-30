# Code review: the crossing skip (decode reuse, kept JSON, Excord's frozen rows), round 3, narrow re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `01279c4e98`.
- **Method:** one brief (sha256 `3c6cb334ca72276ea5c79da50d1b9d7ae97f8762ef257958b85580a758d6a649`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `js/src/binary.js:450-458` — The wrapper matches the engine for ordinary `new Proxy`, a call without `new` (`new.target` is undefined, so it throws `TypeError`), `Proxy.length` (`2`), and `Proxy.name` (`"Proxy"`). Hardening still freezes it: the prelude installs the wrapper, then `capture_intrinsics` and `harden.js` freeze that global function, so `length` and `name` end up non-writable and non-configurable. The `WeakSet` is only in the closure, so later `new Proxy` and `Proxy.revocable` still record. `Reflect.construct(Proxy, [target, handler])` returns a tracked native proxy, and `class extends Proxy` still fails because `.prototype` reads as `undefined`. Two gaps remain. `Proxy.prototype` is an own data property, so `'prototype' in Proxy` is true, `getOwnPropertyNames` lists it, and `delete Proxy.prototype` throws in strict code; the engine has no such property. `Proxy.revocable` is an enumerable, constructable function with its own `.prototype`, so `new Proxy.revocable(target, handler)` returns the record and `Object.keys(Proxy)` is `["revocable"]`; the engine's `revocable` is non-enumerable and not constructable. `Reflect.construct(Proxy, args, newTarget)` also reads `newTarget.prototype` before the body, which the engine's `Proxy` does not. Define `revocable` as a non-enumerable method (`{ revocable(target, handler) { ... } }.revocable`). A JS function cannot drop its own `prototype` property; leaving the value `undefined` is what blocks subclassing. Matching `Reflect.construct`'s third argument needs the engine's `[[Construct]]`, not a JS function.

2. `js/src/binary.js:448-459` — Only proxies created after this wrapper is installed are recorded. `pure.js`, `standard.js`, and the Ibex bindings run before it and do not call `Proxy`; `prelude.js` and the app run after, so their proxies (including `Date` and the Intl format wrappers) are recorded. A proxy created before this assignment, or through the closed-over native constructor, would miss the set and could be kept. Keep the wrapper as the first thing that can construct a `Proxy`.
