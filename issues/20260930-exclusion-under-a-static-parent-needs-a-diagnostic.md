# An exclusion under a static parent is placed elsewhere and wraps nothing; the compiler could say so

**Status:** Open
**Systems:** Contract compiler, kernel (flow)
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1043.000 §8, LLP 1074 §3.6, kernel/src/flow.rs `contexts`, host/apple/tests/it/textflow.rs

A `wrap-flow: both` box is `position: absolute`. Since LLP 1074 it is placed against its nearest positioned ancestor, not its parent, while its wrapping context is still its parent. Under a static parent it lands somewhere else in the containing block and wraps nothing of the parent's text; the kernel refuses auto-height flow there with `FlowRefusal::Context`, whose message now says to give the parent `position: relative`, but a paragraph with a definite height is silently not wrapped, and the error only appears at run time in the journal.

The compiler knows both the exclusion and its parent at lowering (components are inlined first). Refuse, or warn, when a `wrap-flow: both` element's parent names no position and is not lowered `relative` and is not the root: "`wrap-flow: both` wraps its parent's text; give the parent `position: relative`". The Apple textflow test fixture needed exactly this edit (`host/apple/tests/it/textflow.rs`).
