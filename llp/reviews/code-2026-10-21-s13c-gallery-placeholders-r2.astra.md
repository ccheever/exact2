# Code review: contract gallery: a symbol's prop gets a role and a handler's action prop an inert action of its payload (stage 13 C), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `2743aebec6`.
- **Method:** one brief (sha256 `c8cf03b48316cc91906b883a2ebe7933ec2a732e8b2c61531a9b551ea67721f1`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken: the comment on differing payloads corrected; the stem test's prop now names no role alone (chevron, of chevron-left)

VERDICT: RIGHT WITH NITS

1. **nit — [contract/cli/tests/it/gallery.rs:774](/tmp/iosfix-rv-gal2/contract/cli/tests/it/gallery.rs:774):** The comment overstates that differing payloads are a library error. `navigate` accepts zero arguments, so sharing an action with `press` is valid. Add a regression asserting `noop` for this combination in both traversal orders; it directly exercises the corrected conflict handling.

2. **nit — [contract/cli/tests/it/gallery.rs:777](/tmp/iosfix-rv-gal2/contract/cli/tests/it/gallery.rs:777):** Coverage still omits non-primitive payload declarations, generated-name collisions, index/`match` shadowing, and forwarding through `Use`. The existing assertions cover role fallback, aliases, stems, item shadowing, signature reuse, and explicit arguments.

No blocker or should-fix found. The round 1 logic is correct on inspection. Nonempty payload-table entries correspond to handlers; `resize=Ident` is a handler under the compiler’s rules. Payload types and parameter names are valid, and `Use` arguments remain skipped.

Declaration ordering is deterministic. Allocating inert names after components/shapes preserves their names, and an empty declaration set preserves the previous gallery header bytes. Normal compilation paths are unchanged.

Static review only: no files edited, no Cargo run, and the 46-app byte comparison was not independently repeated.
