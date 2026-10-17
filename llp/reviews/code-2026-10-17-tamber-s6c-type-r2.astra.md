# Code review: Type and family tokens, and Expose converted to them (LLP 1121 stage 6 C), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~2..HEAD, the change, Expose's conversion and the first round.
- **Method:** one brief (sha256 `aa5eedbc868d57b98e79e8cd553e77a325cc01e2f7d15ac0f602fcd910a6dee7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a P1: a bound JS family losing a bundled font's alias; P2s on a bound type token's size, a choice's children, a stale token index, spaces around a name, a family-less type token's face check).
- **Disposition:** in the second-round commit.

---

Blocking findings remain.

- **P1 — Bound JS fonts lose bundled aliases.** [host/web-js/src/measures.rs:53](/tmp/rv-s6c-2/host/web-js/src/measures.rs:53) returns authored family names, while bundled faces register as `ExactPlanStackN`. A conditional `font-family` choosing family tokens therefore falls back instead of using the bundled font. Type tokens have the same problem at line 62.

- **P2 — Bound type tokens lose default weight and leading.** [host/web-js/src/measures.rs:60](/tmp/rv-s6c-2/host/web-js/src/measures.rs:60) converts text styles to `rem` before `rows.rs` derives their defaults. With `font=(flag ? "type.headline" : "type.body")`, the subsequent text-style lookup receives `1rem` and returns null. The headline inherits the wrong weight/leading instead of matching native rendering.

- **P2 — Conditional/list children bypass the inherited-axis refusal.** [runner/src/bridge.rs:537](/tmp/rv-s6c-2/runner/src/bridge.rs:537) checks only direct node parents. Children inside `if`, `for`, or `match` belong to region arms. A container with computed `font-family=chosen` can consequently accept a varying token; a pinned descendant then inherits the outer tuple’s family without reselecting it.

- **P2 — Runtime family switches leave stale token entries.** [runner/src/instance.rs:1083](/tmp/rv-s6c-2/runner/src/instance.rs:1083) skips index cleanup when interning a runtime family. After `choice` changes from `textFace` to `Georgia`, editing colour token `ink2` restores the old family: both names share refresh bit 7, and the index still contains `textFace`.

- **P2 — Whitespace around token names drops the family.** [runner/src/bridge.rs:690](/tmp/rv-s6c-2/runner/src/bridge.rs:690) calls `theme::names` without trimming. Lowering and `family_binding` accept `" family.mono "`, but this lookup rejects it. A padded type-token name likewise resolves its other longhands while losing its family.

- **P2 — Bundled-face validation misses a separately supplied family.** [contract/lower/src/type_tokens.rs:138](/tmp/rv-s6c-2/contract/lower/src/type_tokens.rs:138) returns early when the token omits its family. Thus `type.emphasis = italic 700 1rem` beside `font-family="RegularOnly"` bypasses `lower-font-face`, despite the bundled font containing only a regular face.

Existing parser and JS family tests pass, but omit these combinations and transitions. Host launches were not validated; temporary-file probes were sandbox-blocked. The worktree and index remain unchanged.
