# Bundle activation commits before the host accepts the generation

**Status:** Closed
**Resolution:** Fixed by app-wide prepare/accept/commit across core hosts and optional updater adapters; fresh two-session native refusal/success, delayed-draw, launch-integrity and Linux presenter transaction drives passed.
**Systems:** Update store, Apple host, Linux host, Embedding
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1023 D3; LLP 1026 D11; LLP 1031 D1/D8/D11

`Store::activate()` promotes pending to selected, writes the record, and marks
the entry running before returning the plan (`update/src/store.rs:592-608`).
The host has not decoded or applied that plan yet.

Linux then installs the new asset overrides before `reload()`; an error leaves
the store and resolver on the new generation while the prior runner remains
(`host/linux/src/presenter.rs:261-274`). Apple likewise commits in Rust,
installs the selected entry's assets globally, and only then applies the plan
to sessions one at a time (`Session.swift:132-141,190-197`). If one of two
sessions refuses, the other may advance, but the app-owned store, status, and
asset resolver can describe only one generation. Both override maps are
insert-only, so assets removed by a successor remain resolvable too
(`Session.swift:92-99`; `host/linux/src/image.rs:216-235`).

This is more than an ordering bug. LLP 1031 D11 permits each session to pin
the generation it kept, while D1 gives every session one app-owned asset
resolver and one store. Those ownership rules cannot represent two pinned
bundle generations. LLP 1030.001 records the structural choice.

Done when that LLP chooses and the implementation proves one coherent model:
either prepare every session and atomically advance the app/store/resolver only
if all accept, or give pinned sessions generation-scoped resolvers and running
state. In either model, decode/application refusal changes no committed state,
removed overrides disappear, first-pixel blessing names the generation that
actually drew, and two-session success/refusal cases are driven. Related
leaf defects: `apple-updates-activate-discards-the-entry` and
`linux-boot-ignores-entry-assets`.
