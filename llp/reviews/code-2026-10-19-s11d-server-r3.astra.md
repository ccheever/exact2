# Code review: the server half of stage 11 D (base names on the wire, tamber/1.14), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `30e22dbe4a`.
- **Method:** one brief (sha256 `db974cbc4efee11bbbbe292c9f54fe6ef4307f44e7997aaaade8106d1f2d2be5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Nothing blocks this change by inspection.** The round-2 functional findings are fixed. Two nonblocking findings remain:

- **Should-fix — [tamber_serve_base.rs:131](/tmp/rv-s11d-3/contract/cli/tests/it/tamber_serve_base.rs:131): the chain regression doesn’t test an overridden base.** `card.inner → card.pad` reaches unset bases, which already survive in the folded `Length::Named` values. This passes without exercising the new recursive traversal. **Fix:** add `card.outer = gap`, `gap = radius.lg` using the fixture’s existing `radius.lg` override, and assert `bases: ["radius.lg"]`.

- **Nit — [tamber_base.rs:65](/tmp/rv-s11d-3/contract/cli/src/tamber_base.rs:65): “in the order it folds them” overstates the ordering guarantee.** The implementation emits unset bases before set-base references, whose order comes from token declarations. With a default referencing overridden `space.md` and a later branch referencing unset `space.sm`, it emits `["space.sm", "space.md"]`. **Fix:** document this as a deduplicated collection without an encounter-order guarantee. The brief explicitly orders `describe.base`; it doesn’t specify `bases` ordering.

Each round-2 finding is fixed or answered:

| Prior finding | Current disposition |
|---|---|
| Unselected defaults lose bases | Fixed: `fold_kind` records them when `unselected` folds them. |
| Type-size references disappear | Fixed: `Value::Type.size` is recorded. |
| Chains through overridden bases disappear | Fixed: `bases_of` traverses checked token references. Regression coverage remains incomplete above. |
| Repeated snapshots/folds mix replies | Fixed: additions use the handler’s existing snapshot and fold. The extra describe retry and resolve/explain lookups are gone. |
| Quoted-family assertion passes on errors | Fixed: it now asserts the successful token first. |
| Alphabetical column keys | Unchanged and acceptable for a JSON object; all five columns remain present. |
| Terminal mapping precedes host adoption | Answered as the B2 dependency; host adoption remains unfinished here. |

The wire shape matches the brief: nine rows in schema order; all 45 cells read from the kernel; exact source fields; graphical `"full"` and terminal `"0"`/`"<n>cell"`; and override file, name location, and default-value text from the selected design. Exact-name matching holds. When-only declarations and package base definitions are refused.

Unset and overridden resolve/explain responses have the required fields. Param precedence remains intact. Semantic collection excludes quoted strings and lookalikes, includes branches and chains, and deduplicates. Base lengths inside `light-dark()` are refused by the checker. Targets use the same selection as icon `draws`, retaining unknown targets with `column: null`. I found no new stable-input failure for ordinary tokens, params, icons, or valid alternate-file requests.

Tests cover every cell/source, override location, both response forms, unknown targets, unused defaults, type sizes, and host-source mapping. Missing cases include successful alternate-file base overrides, package/param base references, param/base-name collisions, malformed designs, and literal terminal-string assertions independent of the serialization helper.

Both permitted Cargo commands failed **before compilation** because the sandbox denied creating temporary `target…` directories. No tests ran; no files were changed.
