# Code review: held :hover and :focus-visible looks editable, the source map's state parts (LLP 1122 queue item (1)), round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `b6e0727700`.
- **Method:** one brief (sha256 `205d9146d6c27e2f25833e1e8dffc68b240253aa47447f10f6b4f117e97a2fdc`), shared by both reviewers, each blind to the other's review; commits d76b990cb7 (c1), 709c0bb8bc (c2) and b6e0727700 (round 1's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 1 NIT).
- **Disposition:** all taken post-round, in the commit named in the sign-off request:
  - `writtenAs` reads the name at the map's UTF-8 byte column, as `plainAt` does; tested with a two-byte character before a part on its line (1).
  - A conditional class's `:active` part keeps its `state` with no place: `uses` sets `state` at the node's place, agent-inspect sets `style.state` with no `at`; both tested (2).
  - docs/tamber-protocol.md: a border side by the name written at its part, its longhand or `border-color`; a part with no place (a conditional class's) falls back to the node (3).

Nothing blocks. I found two **SHOULD-FIX** issues and one **NIT**.

1. **SHOULD-FIX — `writtenAs` treats a byte column as a JavaScript string index.** [scripts/tamber-inspect.mjs:190](/tmp/rv-dt-q1b/scripts/tamber-inspect.mjs:190)

   Concrete case: a `:hover` line containing `background-color=" #fff" border-color="#111"`, where the space before `#fff` is literal U+00A0. The colour parser accepts that whitespace, but the map’s UTF-8 byte column is one greater than the corresponding JavaScript index. `slice(at.col - 1)` starts at `order-color`, so `writtenAs` returns null and all four border sides lose their edit rung. Use the byte-based extraction already used by [plainAt:305](/tmp/rv-dt-q1b/scripts/tamber-inspect.mjs:305). Ordinary ASCII shorthand/longhand cases are fixed.

2. **SHOULD-FIX — conditional `:active` parts lose their state label in two readers.** [contract/cli/src/tamber_uses.rs:399](/tmp/rv-dt-q1b/contract/cli/src/tamber_uses.rs:399), [scripts/agent-inspect.mjs:122](/tmp/rv-dt-q1b/scripts/agent-inspect.mjs:122)

   Let styles `A` and `B` both declare `:active background-color="ink"`, and apply `class=(flag ? A : B)`. This compiles; HEAD correctly emits `{state:"active", origin:"class:A/B"}` without `at`.

   `uses` now falls back to the node’s location, but its `at` filter also skips assigning `"state":"active"`. Agent inspection similarly attaches the origin but drops the state because both state and location are assigned inside `if (placed)`. Thus these readers make the part indistinguishable from an ordinary press-paint binding. Preserve the state independently of whether a location exists.

   The selection join handles this case safely: its ordinary row retains `state:"active"` with `at:null`, and its held row uses the owner’s location while refusing the conditional-class edit.

3. **NIT — the protocol description still gives the pre-fix border rule.** [docs/tamber-protocol.md:1425](/tmp/rv-dt-q1b/docs/tamber-protocol.md:1425)

   It says a border side edits `border-color` whole. A directly authored `border-top-color` now correctly produces that longhand instead. Document both possibilities. Nearby location wording should also cover an existing conditional part without `at`, whose held `source` falls back to the node.

The other round-1 fixes hold by inspection. `class_spans` includes the class’s written state attributes, and longhand expansion preserves their spans, so animation composition and grouped-list insertion no longer misattribute looks. The no-map `continue` follows `self.attr()` and skips only provenance construction; it does not skip bindings or `set_looks`, and mapped origins remain aligned with bindings.

I found no regression in `rowRung`’s missing-`attr`, layout, unreadable-file, or package guards; non-side and `:active` property naming remains correct. The map keeps one binding per row, separates base fields from state parts, and joins parts by `(row, state)`. I found no hover/base edit collision or change to plan construction calls or their order.

Read-only review; no builds or test suites run. The working tree is unchanged.
