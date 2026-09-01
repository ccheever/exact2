# Button primary text sugar is rejected

**Status:** Closed
**Resolution:** A button primary expression now canonicalizes to a real accessible text child.
**Systems:** Contract compiler
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017.001

LLP 1017.001 explicitly retains primary-argument sugar such as
`button "Post" press=submit`. `contract/lower/src/tags.rs:88-93` assigns button
no positional target, so lowering instead reports `lower-positional: button
takes no positional argument`.

Lower the primary string to the button's text child/label exactly as the
language document specifies, or amend the governing LLP if that syntax was
intentionally removed. Add a corpus fixture that compiles and exposes the
accessible/button text on every host.
