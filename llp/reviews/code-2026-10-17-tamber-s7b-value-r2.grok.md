# Code review: The app's value of a param (LLP 1121 stage 7 B), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `55186696d..7cb7f1cbc` as reviewed (rebased since onto stage 7 A's re-check fixes), B and its first round.
- **Method:** one brief (sha256 `8be31b9be3867f50ed07f30817fdb0897564b706a470a7c7d9239031198b773f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (P1s: token-valued and dominated ranges and their lines, an ambiguous line the server commits around, the rename proof without params, icon aliases in pinned; P2: the protocol page on `uses`).
- **Disposition:** the last round, applied without re-review in `b2dfdb9f6`. 1: each written value folded on its own at its line. 2: entry errors refused by the server's writes and reads; unset by the line's own spelling repairs (rewriting the line to the full name was not built: unset then set does it). 3: the rename proof lays the params. 4: pinned follows icon aliases on the parsed design and params' values. 5: the claim dropped.

---

The shared layout is in place, and both declined round-1 items hold. Four holes remain in the checks around that layout.

1. **P1 — A token-valued or non-literal override is not range-checked at its own line.** `contract/lower/src/params.rs:271` walks only the winning `token.values`, and `:262` charges every failure to one span (the params name line, or the declaration). `written_ranges` at `:354` parses only bare `Npx` / `Nms` / `Ns` / numbers. `contract/cli/src/tamber_ops.rs:205` stores that same name-line span, and `contract/lower/src/theme.rs:264` does too. The stage 7 B note says each written value, a `when` included, is checked at its own line, and a value that names a token is checked after folding. Round 1 claimed that; only a bare literal got its own line.

   A dominated `when` that names a token is accepted by `set-param` and by `tamber check`. With `PostButton.radius` ranged `0..40` and `radius.huge = 80px`:

   ```
   params
     PostButton.radius = 8px
       when density=compact: radius.huge
       when density=compact size=small: 8px
       when density=compact size=large: 8px
   ```

   `radius.huge` never wins a tuple and is not a literal, so the file builds. The same hole is a base `PostButton.radius = radius.huge` covered by in-range `when` lines, and a dominated `when` of `1rem` or `calc(80px)`.

   A winning `when` is refused, but on the wrong line. `PostButton.radius = 8px` / `when density=compact: radius.huge` (or `: 1rem`) reports `tamber-param-range` or `tamber-range-unit` on the `8px` line. The message names `80px`, so the failure is visible and the bad value does not ship. A winning base `1rem` or token is attributed correctly, because that name line is the value line.

   Fold every written value, including ones `unselected` drops, and attach that value's own span. The comment on `folded` (`:249`) currently overstates this.

2. **P1 — An ambiguous short line cannot be updated or removed, and the server will commit a file the build refuses.** `param_names` (`contract/cli/src/tamber_ops.rs:154`) treats a line as the param's only when `resolve` succeeds. `named` (`tamber/src/params.rs:89`) returns ambiguous for that short name, and `set_param` (`tamber/src/edit/params.rs:25`) matches the raw spelling. The compiler still errors at `contract/lower/src/theme.rs:277`. `param_values` only runs `laid` and `check_design`, which drop the ambiguous line, so it never surfaces `tamber-param-ambiguous` or `tamber-param-twice`.

   Two components named `Card`, only `a.contract#Card` declaring `inset`, and a design that already has `Card.inset = 9px` (written when one `Card` existed). `set-param` of `a.contract#Card.inset` to `4px` appends a second line. The server accepts it: `laid` applies the full name and skips the short one. The build still refuses on `Card.inset`. `unset-param` of `Card.inset` is refused as ambiguous, and `unset-param` of the full name does not see the short line, so the tool cannot repair the file. A hand-written second line (`tamber-param-twice`) is invisible the same way: an unrelated write is committed while the build stays refused.

   When one param owns that short spelling, treat the short line as its line and rewrite it to the full name. When several params share it, refuse the transaction while the ambiguous or twice line remains. Run those two checks inside `param_values`.

3. **P1 — `rename` still proves the design without params.** `tamber/src/edit.rs:231` calls `crate::check`, and `step` has no params argument. A dial no laid token reads is `tamber-dial-inert`, so both sides fail and every rename is refused with "a rename needs a design that checks before and after it".

   ```
   dials
     pad: length = 8px  in 0px..32px

   theme
     radius.md = 12px

   params
     PostButton.radius = pad
   ```

   `set-param` of `calc(pad + 1px)` succeeds. Renaming `radius.md` to `radius.lg` is refused, even though `apply_all_with` would lay the params and the later check would pass. Round 1's rename fix does rewrite values and `when` lines. This proof was left outside that path.

   Thread the params into the proof and fold with `laid`, the same way `apply_all_with` does.

4. **P1 — `pinned` ignores an icon alias, so a set value that still follows the default icon is marked pinned.** `reads` (`contract/cli/src/tamber_params.rs:100`) walks `token.names` only. An icon has no such field, and `laid` has already overwritten the alias with a literal before `reads` runs. Token chains are followed. Icon chains are not. The stage 7 B note includes icons the default names, directly or through another token.

   ```
   icons
     icon.home = icon(sf "house")
     icon.alias = icon.home

   params
     App.glyph = icon.alias
   ```

   `App.glyph`'s default is `icon.home`. After the alias resolves, `reads("icon.alias")` finds the icon by name and not `icon.home`, so `pinned` is `["icon.home"]` while the set value still follows it. The same false pin happens when the set value is a param whose own value is `icon.home`.

   Walk icon references on the parsed design, before alias resolution, and on param defaults and overrides. Record referenced names on icons the way tokens carry `names`.

5. **P2 — The protocol page contradicts itself on what `uses` accepts.** `docs/tamber-protocol.md:195` says the components `token` hash is something "`uses` takes". `:201` says `resolve`, `explain`, and `uses` take a full or short name and answer by the full name. Both inputs work. Drop the "uses takes it" claim. The hash stays the mapping in `token`, which is what describe is for.

Round 1's other fixes hold: one `laid` everywhere, icon aliases copied after the override, one spelling per param inside a transaction, an ambiguous short name refused by `resolve`, literal `when` values range-checked on their own line, unknown names warned, the block found with its comment and indent, and icon `explain` pointed at `describe` and `resolve`. Motion uses stay source literals, as motion tokens do: the rows are written before any instance exists. An unused duration or easing param stays `rebuild`: the planner never sees those row sites, and the name is gone before retention, so the extra rebuild is the safe class.

RIGHT WITH CHANGES
