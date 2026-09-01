# Action prefixed typos become action types

**Status:** Closed
**Resolution:** Restricted action types to the declared bare action grammar and rejected prefix typos with stable type diagnostics.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1006

`contract/types/src/lib.rs:171-175` recognizes an action type with
`strip_prefix("action")`. Any identifier beginning with that text therefore
falls into the action parser; `actionfoo` is accepted as bare `action()` rather
than reported as an unknown type.

Require the exact `action` token followed only by the declared action-type
grammar. Add accepted `action`/`action(...)` fixtures and reject
`actionfoo`, `actionable`, and malformed parameter lists.
