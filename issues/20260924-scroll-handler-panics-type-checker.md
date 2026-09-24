# A one-parameter scroll handler panics the type checker

**Status:** Open
**Systems:** Contract compiler
**Severity:** P1
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1006

`scroll` always contributes two payload types, `scrollLeft` and `scrollTop` (`contract/types/src/component.rs`). The loop indexes `ct.actions[ai][last]` whenever the handler passes fewer explicit arguments than the action has parameters, and it never checks that `last` is in range. For one parameter and no explicit arguments, `start` is 0 and the second payload reads index 1.

```
component App
  state n = 0
  action onScroll(x: number) writes n
    n = x
  view
    scroll scroll=onScroll height=100
      text "hi"
```

`contract build` of that file panics: `index out of bounds: the len is 1 but the index is 1` at `component.rs:376`. Reproduced with `target/debug/contract`. The same handler with zero parameters is refused cleanly (`analyze-handler-arity`). `scroll=onScroll(n)` also returns a diagnostic. Analysis never runs in the one-parameter case.

Bound the payload walk by the parameter list and return the arity diagnostic. Done when this file is a typed refusal, and a two-parameter `scroll` handler still type-checks.
