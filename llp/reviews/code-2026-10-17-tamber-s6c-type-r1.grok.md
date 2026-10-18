# Code review: Type and family tokens, and Expose converted to them (LLP 1121 stage 6 C), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the change and Expose's conversion.
- **Method:** one brief (sha256 `de5a5868116498e6917c7b11b76c9c1c1d77c380ba5936edd81845a7f61c0283`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (a P0: undotted names interned as font families, Expose's faces never resolving natively; a P1 on stack matching; a P2 on a type token's shipped-face check; P3s).
- **Disposition:** in the first-round commit.

---

I'll review the stage 6 commit C range read-only: the spec, the type and family token changes, and how Expose uses them. First I'm locating the repo and the review criteria.The review is read-only and scoped to `HEAD~1..HEAD`. I'll locate the repo and the stage 6 commit without touching the worktree.HEAD is the Expose token conversion; the commit before it is the type and family token work. I'll review both against the stage 6 spec.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is ready to write once the blocker and the secondary findings are checked against the current lines. I'll confirm those sites, then report.**Blocking.** On the native runner, Expose's family tokens never resolve. An undotted token name is interned as a runtime font family, so the kernel never sees the token.

## P0 — Undotted type and family token names are interned as font families

`runner/src/instance.rs:1081-1108` interns every `font-family` string before `set_binding_style`, whenever fonts are armed (`runner/src/runner.rs:1065` does that on the first frame). `RuntimeFonts::intern` (`runner/src/bridge.rs:23-33`) returns a new stack id as soon as `runtime_family_members` is non-empty. `is_css_family_name` (`plan/src/lib.rs:649-724`) rejects a `.` and accepts a single ident that is not a CSS-wide keyword. `set_runtime_font_style` (`runner/src/bridge.rs:578-586`) then writes that id as a number. The token branch in `set_style` (`runner/src/bridge.rs:617-630`) runs only when intern returns `None`.

Expose names its faces `textFace`, `liltFace`, and `glyphFace` (`apps/expose/app.contract:91-93`). The root is `font-family="textFace"` (`app.contract:258`), and the same three names are written about 49 times in `app.contract` and `parts.contract`. Boot interns `"textFace"` as a runtime family whose name is the token, with no file (`host/apple/src/measure.rs:227-258`, `host/web/src/host/fonts.rs:25-32`). The declared stack `"Exposure Sans"` (`app.contract:30-35`) is never selected, and the same happens for Exciting Lilt and Exciting. Measure and paint both use that missing family, so the screens are not the previous ones on macOS, iOS, or Linux. Bold Text does not step it (`runner/src/runner/root_font.rs:91-95`): the first member is not a generic.

`font="body"` has the same hole: the family longhand's value is the token name (`contract/lower/src/type_tokens.rs:101-104`). `font="type.body"` and `font-family="family.mono"` survive only because of the dot. `contract/cli/tests/it/tamber_type.rs:47` uses `family.mono`, so intern fails and the test passes. The JS target calls `set_binding_style` directly (`host/web-js/src/style.rs:177-178`) and can still match.

The as-built line is the opposite of this path: the runner is supposed to pass a font-family string that names a token to the kernel rather than intern it (`llp/1121-tamber.rfc.md:2812`).

## P1 — Even a dotted name can bind the wrong stack, or none

Runtime lookup re-keys the token text with `family_key` (`runner/src/bridge.rs:416-420`, `kernel/src/style/theme/typeface.rs:142-173`) against `plan_stacks` (`runner/src/bridge.rs:480-501`). The compiler keys the stack it declared by the original family string (`contract/lower/src/type_tokens.rs:59-82`) and rewrites with `family_names` (`contract/lower/src/fonts.rs:520-529`). Those two keys disagree.

*   `family.named="\"monospace\""`. `family_names` keeps a quoted generic as a custom family (`fonts.rs:521-522`) whose plan name is `monospace` (`fonts.rs:304`). `plan_stacks` then keys that stack `monospace`, the same key as generic stack 0–7, and `find` returns the generic. `font-family="family.named"` paints generic monospace. The literal `font-family="'monospace'"` stores the custom stack id and does not re-key.
*   `family.system=-apple-system`. The literal path rewrites that to `system-ui` and stack 0 (`fonts.rs:523-524`). The token text stays `-apple-system`. `family_key_of` does not rewrite it, `entry` returns `None` (`typeface.rs:127`), and `measured` requires a `#id` (`kernel/src/style/theme.rs:297`, `typeface.rs:113-116`). The row is cleared. `font-family="-apple-system"` still resolves.

Expose's `"Exposure Sans"` would match this key if the string reached it. P0 stops it first.

## P2 — A type token on `font=` skips the bundled-face check

`font_use` only reads an attribute named `font-family` (`contract/lower/src/fonts.rs:173-197`). `lower-font-face` only sees that `FontUse` plus separate `font-weight` and `font-style` attributes (`contract/lower/src/values.rs:845-887`). `font="700 italic 16px \"Exposure Sans\""` can name a weight or style the bundled font does not ship, and v1 "never synthesizes" is not enforced. The same weight as `font-weight` beside `font-family="\"Exposure Sans\""` is refused. Expose's current rows are the second form, so this does not change Expose.

## P3

*   Using a family token as a colour says "`{name}` is a type token, not a colour" (`tamber/src/fold.rs:256-259`). `Named::Type` and `Named::Family` share one message.
*   A family token inside a type or family value is followed only when the name contains a `.` (`tamber/src/typeface.rs:240-245`). `type.reading = -exact-body textFace` is a font member named `textFace`, not Expose's token. The as-built text allows another family token (`llp/1121-tamber.rfc.md:2794`). Expose does not use this form.
*   `font_use` splits `family_key_of(stack)` on every comma (`fonts.rs:195-196`), so a declared family whose quoted name contains a comma is not the font the face check sees.

## Held

Ruling (2): the session shape includes a type token's longhands and family, and a family token's stack (`tamber/src/session.rs:397-405`). A value-only edit of the same longhands stays live. Changing the set is a rebuild.

Ruling (3): type and family resolve fields are optional under tamber/1.9. The version is not bumped.

Ruling (1): a varying type or family token on a node with children is `tamber-inherited-axis`. The check treats any child as an element child, so `for`, `if`, and a component slot are refused too. A text with no children is re-selected per tuple (`kernel/src/style/theme.rs:287-293`). Expose's root family does not vary, so the container is allowed.

`fontShorthand` still fills weight and leading only where the token writes none. A written leading stays the token's. The JS refusal of a varying or bound type or family token is the stage 6 C limitation already queued, and a dev edit of one rebuilds.
