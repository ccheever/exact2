# Code review: Type and family tokens, and Expose converted to them (LLP 1121 stage 6 C), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~2..HEAD, the change, Expose's conversion and the first round.
- **Method:** one brief (sha256 `aa5eedbc868d57b98e79e8cd553e77a325cc01e2f7d15ac0f602fcd910a6dee7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (a P1: a bound JS family painting the declared name, not the shipped face; a P2 on splitting a quoted comma).
- **Disposition:** in the second-round commit.

---

I'll review round 1's fixes and the stage 6 commit C range read-only, starting with the review skill and the commits themselves.The review stays read-only. I'll inspect the round-1 commit, the stage 6 spec, and the files those fixes touched.Auto-compacting conversation (81% full)...
Conversation compacted.
I’ll finish the round-2 review from the code as it stands, checking each round-1 fix and the cross-cutting cases before reporting.The JS bound path for a shipped face is the one defect that should block calling this done. Static rows, the native runner, and Expose’s screens hold.

## Findings

**[P1] A bound type or family token on the JS target paints the declared family, so a shipped face never loads — `host/web-js/src/measures.rs:54`**

`font_part` returns the authored family text (`InterVariable`, `"Inter", system-ui`), and `map` (`measures.rs:242`) always JSON-stringifies it. The font-family setter then takes the string branch (`host/web-js/src/rows.rs:251`) and calls `fontFamily()`, which only rewrites unquoted generics and `-apple-system` (`host/web-js/font-family.js:11`). It never consults `fontAliases`. `@font-face` and a static `font="type.a"` both use `ExactPlanStackN` (`host/web/src/css/fonts.rs:21`, `host/web-js/src/faces.rs:38`), because the kernel binds the plan stack id and `font_family_names` aliases any family that has faces. A literal stack index still hits that table; a token name is a string, so it cannot.

`text "a" font="type.body"` and `text "b" font=(big ? "type.body" : "type.caption")`, with `type.body`’s family a face the app ships: the static node loads `ExactPlanStackN`; the bound node sets `font-family` to the declared name. The browser falls through to the next family. Layout widths on the JS target follow the fallback; native measure and paint follow the shipped face. Same bug for `font-family=(c ? "family.ui" : "family.text")`. Generics and unquoted `-apple-system` happen to match, which is all `measures_tests.rs` asserts (`"type.b":"system-ui"`).

**[P2] The shipped-face check splits a comma that belongs to one family name — `contract/lower/src/type_tokens.rs:143`**

`family_key` quotes a family member, so `"A,B"` keys as `"a,b"`. `bundled_faces` then `split(',')` and keeps only segments that start with `"`, and `font_use` does the same split plus `trim_matches('"')` (`contract/lower/src/fonts.rs:195`). `"a,b"` becomes `a` and `b`. A type token `italic 700 16px "A,B"` whose shipped font `A,B` has no bold italic face is not `lower-font-face`. If a different font named `A` or `B` is declared, the check runs against that font instead. Stack matching itself is fine: `plan_stacks` compares the whole key, and both sides keep `"A,B"` as one member.

## Round-1 fixes that hold

- A token name is not interned (`family_binding` / `names_font_token`). `set_style` stores it as text. The runner table rewrites the family to `#id` via `family_key`, including quoted generics, case, `"A,B"` as one member, and unquoted `-apple-system` → `system-ui`.
- A one-word family token, including inside a type token, folds to that stack and records the dependency. A change to the stack or to the longhands a type token writes is a rebuild; a style, weight, size, or leading change is live, and `set_tokens` re-sets the indexed rows.
- A varying type or family token on a node with element children is `tamber-inherited-axis`. A runtime name is refused when any plan node’s parent is that node. The document fold returns unsupported once the plan has axes, so it never ships that style. On a text, the kernel re-reads the tuple’s slice.
- `oblique` and `normal` are refused. A text style’s capitals are read (`-EXACT-BODY`). A value no `font` longhand can read is `tamber-kind`.
- `lint_scope` binds tokens by kind. `font="type.x"` is not parsed as a shorthand, and a font-family attribute is not style-checked as a colour. Wire fields stay optional.
- The JS dev build marks every type and family token inline, so a value edit rebuilds, and a varying one is refused in every build.

Expose’s `font-family` attributes are literal token names (`textFace`, `liltFace`, `glyphFace`), so they take the static kernel path. This bound-row bug does not move those screens.

The tests never put a shipped face through a bound JS row, and never run a comma-name through `bundled_faces` or `font_use`.
