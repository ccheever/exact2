# Enforce the session owner when resolving document handles

**Status:** Open
**Systems:** Data documents, Native storage, Apple sessions, Hermes storage
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1031 D2, LLP 1069.010 D1 and slice 3, data/src/documents.rs

The document table records an `owner`, but lookup does not use it. `mint` uses the owner for deduplication and `forget` uses it for teardown; `resolve(path)` accepts any live process-wide entry. The Rust storage worker and Hermes resolver call this global lookup without a caller identity.

Verified with two minting owners (101 and 202) and an independent activated `Storage<DataSource>` with only `fs.read doc:/`: the recipient can read handles minted by either owner. The identifiers are sequential, and `doc:/<n>` can reveal the entry name through readdir. Ending owner 202 removes its handles while owner 101's handles remain readable by the same recipient.

Slice 3 describes handles as owned by a session; Apple's security scopes also belong to that session. Currently that ownership controls lifetime only. Another window's module can discover and use a selected document without receiving the handle through its own route. This also makes document capability behavior depend on whether sessions share a process or a browser page.

Carry the document owner through storage configuration, continuations and the Hermes resolver, and verify it at lookup. If sharing is intended, make that an explicit transfer or app-scoped policy and reconcile the session-owned design; do not leave the owner field suggesting an admission check that does not exist.

Acceptance: two live sessions with the same broad document grant cannot enumerate or use each other's handles under the session-owned policy; each can use its own. Teardown and queued work preserve the same ownership rule.
