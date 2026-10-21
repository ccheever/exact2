# Code review: a folded :disabled or :checked row read as its base, its parts beside it (LLP 1122 queue item (2)), round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `4edf5b8428`.
- **Method:** one brief (sha256 `472a8c152aaa9934332fa9bbde4c25e8bfbd8fef2257f108aa3f54d445ad9ab4`), shared by both reviewers, each blind to the other's review; commits 2db62df4e4 (c1), caed257141 (c2) and 4edf5b8428 (round 1's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 1 NIT).
- **Disposition:** all taken post-round, in the commit named in the sign-off request (and a narrow verification round on it):
  - (1) as Astra's 2: reverted, both parts kept.
  - (2) as Astra's 3: the param by `#<component>.<name>`, whatever its identity's home.
  - (3) a part's edit is not refused for a param the host's row reads (the base's `unread` alone); tested.

Nothing blocks. Round 1’s fixes hold for the cases the records name. Two correctness gaps are worth fixing; neither is a wrong write or a broken plan.

The param base is named from the source text. `writtenToken` (`scripts/tamber-inspect.mjs:473-478`) takes `param.<name>` off the rung and looks up `{rung.file}#{rung.component}.{name}`. The join test covers the disabled branch with empty host names (`scripts/tamber-inspect.test.mjs:100-102`) and a shared component name whose `short` is null (`:103-105`). A miss sets `unread`, and `editable` (`scripts/tamber-inspect.mjs:341`) refuses with “not read yet” (`tamber-inspect.test.mjs:106-107`). For an ordinary app file, `rung.file` is the same relative path `home()` returns (`contract/cli/src/sources.rs:286-287`), which is the `app.contract#Card.fill` the test uses.

A part’s edit stays its own. The part calls `writtenToken(..., true)` (`tamber-inspect.mjs:454`, `:475`), which keeps the source text and clears names, so the param check at `:361` does not see a name. The part that writes `param.fill` is a template rung with `state: 'disabled'` and no `param` (`tamber-inspect.test.mjs:111-112`).

One part a state holds for two full literals. `fold.parts.retain` (`contract/lower/src/states.rs:521`) drops an earlier part of the same state, then the later span is pushed. `a_folded_row_has_one_part_a_state` (`contract/cli/tests/it/source_map.rs:1017-1050`) has the class’s `:disabled background-color="#111111"` and the node’s `:disabled background-color="#999999"`, and expects one state, the node’s line 6. The same longhand is one inspector key, so a class `border-color` and a node `border-top-color` collapse that side to the node’s part.

`color` and the tint go through `WRITTEN` (`tamber-inspect.mjs:185`): `text_color` → `color`, `tint_color` → `-exact-tint-color`. The join test asserts the color part (`tamber-inspect.test.mjs:114`). Tint is that same lookup, and it matches `RUNNER_ROWS` (`states.rs:33`) and the inspector’s `CSS` entry for `tint_color` (`apps/design-tool/inspector.ts:22-24`).

A role spelling keeps its token. `writtenToken` (`tamber-inspect.mjs:480`) keeps a host name when the written text is that name or the same text with a leading `-exact-` removed, which is `editable`’s check (`:359`). The join test keeps `secondary-label` beside the template rung for `-exact-secondary-label` (`tamber-inspect.test.mjs:109-110`). The helper still takes that name from the host’s current names (`tamber-inspect.mjs:467`).

A placeless part uses the node. `const at = p.at ?? node` (`tamber-inspect.mjs:456`) copies `file`, `line`, `col`, and `end_col`. The conditional-class fixture expects the node’s place and a null rung (`tamber-inspect.test.mjs:116-118`).

A part row’s value is `p.editable.text` (`inspector.ts:109`). A template or state rung carries the written text. The contract fixture remaining a style `set-state` part is the recorded disposition; the own part’s template edit is the join test.

On the regression checks:

A class base is origin `class:Name`. `rowRung` returns a style rung with `file` and `text` and no `component` (`tamber-inspect.mjs:270-272`). A style that writes `param.` is refused at compile (`contract/lower/src/params/rewrite.rs:513-518`), so that text never reaches the param lookup. The Tick fixture still gets the style rung on `#ffffff` and `set-state` on the `:checked` part (`tamber-inspect.test.mjs:119-122`).

The new `unread` is set only for a base whose whole written value is `param.<name>` and whose constructed full name is absent from the params map (`tamber-inspect.mjs:474-478`), and it is passed only into the base’s `editable` call (`:449`). A literal base stays on the token path (`:480-481`). Where `rung.file` really is the identity’s file, the row this refuses is the old template edit of the text `param.fill`, which is the recorded fix. A part is called with the host’s `unread` only (`:454`).

`retain` changes `Fold.parts` only. The tuple pushed into the folded attributes is still the name, the `choose` value, the span, `written`, and `by_class` (`states.rs:523-545`). The map half is not part of the plan (`contract/lower/src/sites.rs:1-5`). The hash was not re-run. The fold loop is `:checked`, then `:disabled` (`states.rs:476`); `retain` drops only the state being folded, so a `:checked` part stays ahead of `:disabled`. `:active` removal searches indexes `>= pressed` (`states.rs:549-557`), after the fold slice.

`authored` reads `AUTHORED` before `WRITTEN` (`tamber-inspect.mjs:183`). A held `:active` row uses the `AUTHORED` table on its own (`:420`). Look rows are `background-color`, `border-color`, `opacity`, and `box-shadow` (`states.rs:37`). A border side returns from `writtenAs` before either table (`tamber-inspect.mjs:190-191`).

## Findings

1. **SHOULD-FIX.** `retain` drops a same-state part that is still the painted value when the later part has a `none` arm. `over` (`contract/lower/src/states.rs:170-172`) replaces `Expr::None` with `under`, and `choose` (`:272-278`) always builds that ternary when `under` already has a value. On the second part of one state, `under` is the previous fold (`:484-489`), then `retain` (`:518-523`) deletes that earlier part. The comment there says the value never shows it. The source-map test (`source_map.rs:1013-1050`) only covers two full literals, where that is true.

   ```
   style Card
     background-color="#ffffff"
     :disabled
       background-color="#111111"
   component App
     state flag = true
     view
       box class=Card disabled=on
         :disabled
           background-color=(flag ? "#999999" : none)
   ```

   Disabled and `flag` false paints `#111111`. The value is `disabled ? (flag ? #999999 : (disabled ? #111111 : #ffffff)) : …`. The map keeps only the node’s line. The inspector can edit the expression, whose `none` arm is what reveals the class color, and it has no part for the `#111111` that is on screen. A later part that is a bare `none` is the same hole with the class color showing whenever the state is on. Plan bytes are unchanged: `retain` never touches the value expression.

2. **SHOULD-FIX.** The constructed param name is the rung’s app-relative file, and the param’s full name is the component identity. `full_name` is `{identity}.{param}` (`contract/lower/src/params.rs:40-41`) and `identity` is `{home}#{component}` (`contract/cli/src/sources.rs:731`). `home()` (`:255-284`) uses a nested package’s `package.json` name for a vendored or linked package. `writtenToken` (`scripts/tamber-inspect.mjs:476-478`) builds `{rung.file}#{rung.component}.{name}`, and `rung.file` is `ownFile`’s path under the app (`:237-239`, `:262`, `:279`).

   ```
   vendor/ui/package.json    { "name": "@acme/ui" }
   vendor/ui/button.contract
     component Button
       params
         fill: color = "#0a7aff"
       view
         box background-color="param.fill"
           :disabled
             background-color="#999999"
   ```

   The params map has `@acme/ui/button.contract#Button.fill`. The rung’s file is `vendor/ui/button.contract`, the lookup misses, and `unread` is forced. The base is refused with “not read yet” while the server has read that param. With the control enabled, the host’s names already carry the real full name; `shown` (`tamber-inspect.mjs:448`) replaces that token before `editable` runs.

   A file `ownFile` rejects (path contains `/node_modules/`, or it is outside `own`) never gets a `text` (`:263`). `writtenToken` then returns an empty token (`:472`) and clears the host’s param name the same way. The folded row becomes “a package’s component: read-only here”. The same param on a row with no `:disabled` or `:checked` part still resolves: that row never calls `writtenToken`, and `editable` (`:361-363`) matches the host’s full name.

3. **NIT.** A literal part is refused while the host is showing an unread param. The part’s `editable` call receives the host row’s `unread` (`tamber-inspect.mjs:454`), and `editable` returns immediately (`:341`). The new `base.unread` flag is only on the base call (`:449`).

   `Card` as above, params map empty (a file change clears it until the next read, `scripts/tamber-server.mjs:386-388`), control enabled so the report’s token is the IR param. The `:disabled` row is the literal `#999999`, and its edit says “it reads a component param this server has not read yet”. Selecting the same node while disabled, so the host names are empty, still offers the template rung for `#999999`.
