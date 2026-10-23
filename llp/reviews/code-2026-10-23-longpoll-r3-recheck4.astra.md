# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check 4 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `bf54bb3be6`.
- **Method:** one brief (sha256 `4a7cb78121a952c7cb093b936323559ec3e09596eac47ec8a1b11851a1c91d88`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [data/src/placed.rs:425](data/src/placed.rs:425), [data/src/mixed.rs:347](data/src/mixed.rs:347): Worker continuations still bypass validation. `Placed::turn` consumes `Request::continuation(...).long_poll(...)` and executes `work()` before envelope validation; the host sees an unannotated outer continuation. `Mixed` likewise dispatches a worker child’s continuation unchecked. Validate before consuming either request.
- [host/terminal/src/requests.rs:68](host/terminal/src/requests.rs:68): The refusal is recorded but never delivered: terminal never calls `take_request_refusal`, and `land_requests` only drains completed work. The refused resource/background ticket remains pending. Keeping `ordered` also creates no fence here: the next ordered continuation still dispatches.

**Should-fix**

- [host/render/src/lib.rs:909](host/render/src/lib.rs:909): Semantic long-poll refusals still enter `held.busy`, producing HTTP 503 instead of the renderer’s semantic-refusal handling.

**Nits:** None.

Direct runner handoffs checked:

| Handoff | Admission before dispatch |
|---|---|
| Apple: [host.rs:809](host/apple/src/host.rs:809) → [abi.rs:263](host/apple/src/abi.rs:263) | Yes |
| Linux, Windows, Android: [host.rs:783](host/linux/src/host.rs:783) → [presenter.rs:769](host/linux/src/presenter.rs:769) | Yes |
| Web wasm: [host.rs:1002](host/web/src/host.rs:1002) → [emit_request:1055](host/web/src/host.rs:1055) | Yes |
| Renderer: [lib.rs:852](host/render/src/lib.rs:852) | Yes; classification issue above |
| Terminal: [requests.rs:64](host/terminal/src/requests.rs:64) | Own work prevented; settlement/fence issue above |
| Page enumeration: [pages.rs:65](host/render/src/pages.rs:65) | Yes, including `Again` |

Data seams checked: Rust ABI [encode:285](logic/abi/src/lib.rs:285)/[decode:530](logic/abi/src/lib.rs:530), [JS wire:96](js/src/wire.rs:96), [worker envelope:269](data/src/envelope.rs:269), and [storage rewriting:89](data/host/src/lib.rs:89) validate. Web JS Rust/TypeScript streams reach [http-body.js:385](host/web/http-body.js:385). Worker continuation shortcuts are the exception above. Remaining production `take_requests` matches concern canvas images or platform shell queues.

The fence assertion now distinguishes both ordering cases. Both permitted test commands stopped before execution because the read-only sandbox denied Cargo writes.
