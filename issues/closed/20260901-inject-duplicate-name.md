# `inject` is not in the duplicate-name set; inject vs prop vs state is last-write

**Status:** Closed
**Resolution:** Duplicate inject declarations now produce a stable type refusal.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017 P4a (provide/inject)

`check_component` dedupes props, state, derives, resources, mutations, actions — not `injects` (`contract/types/src/lib.rs`). `props x` plus `inject x` type-checks. At inlining, inject overwrites the prop in `child_subst`; a later state/action of the same name overwrites the inject. The provide is dropped. Two injects of the same name also silently keep the last.

Fix: put inject names in the same `seen` map (`type-duplicate-name`). Refuse `inject` that collides with a prop. Fixture: `props x` + `inject x` is a typed refusal.
