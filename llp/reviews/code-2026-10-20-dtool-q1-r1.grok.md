# Code review: held :hover and :focus-visible looks editable, the source map's state parts (LLP 1122 queue item (1), c1 and c2), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `709c0bb8bc`.
- **Method:** one brief (sha256 `47f231dbf86268ecdbaefd39d662bc474abdf2a10cd64610342aeff900160729`), shared by both reviewers, each blind to the other's review; commits d76b990cb7 (c1) and 709c0bb8bc (c2). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 1 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - (1) as Astra's 2: `class_spans`, the same test as a base row's; the animation case tested (the grouped-list direction is the same test).
  - (2) as Astra's 1: a side named as written at its place.
  - (3) a conditional class's part (look or `:active`) says no `at`, as its base row says none; `uses` and the held row's `source` fall back to the node; tested; docs/reference.md says so.

Nothing blocks. The map keeps one binding per row, keyed by `(row, state)`, and these two commits do not change plan bytes. Two correctness gaps fail closed or mis-label the rung; neither writes a hover row at the base row's place.

What holds:

- A row the base writes and one or more parts write is one binding: base `origin` / `at` / `attr`, and one `states` entry per part. Hover and focus-visible on the same row are two entries. Lookup is by state (`asPart` in `scripts/tamber-inspect.mjs`, `sites` in `contract/cli/src/tamber_uses.rs`).
- Where nothing rewrites the attribute list, the class part and the node's own part agree with the plan. `Folded::looks` keeps the later longhand (`states.rs` retain-and-push; the node's rows come after the class's). The map records that winner's flag.
- `:disabled` and `:checked` still fold into base rows. They never enter `looks` or `states`.
- `focus-visible` is the map spelling and the hold spelling (`HoldState::name`, the web hold). The runtime look object is `focus_visible` (`runner/src/agent/looks.rs`); both hosts read that key and emit `state: "focus-visible"` before the join.
- No reader still reads a row-level `state`. `asBase` requires `origin`, so a press row or a look-only row is not treated as a base. `agent-inspect` `attach` skips a binding with no base origin and no `:active` part. `motion_instances` matches base `at` / `attr` only, so a missing base simply does not match.
- Held row with a part: `source` and the rung are that part (`template` plus `state` when `origin` is `own`, `state` rung when it is `class:<Name>`). Held row with no part in that state: `source` is the held node's element, and `rowRung` is not called, so the base `at` is not an edit address. The selection's own rows prefer `asBase`, and fall back to `asPart` only for `active`.
- Stage 9 E: `held.site` selects the owner map node; the held rows are joined from that node's bindings. The selection `source` stays the inner node.
- `placed` includes every part file, so a moved `:hover` file makes the selection `readOnly` and the held rung is withheld. `sitesAt` matches any part's `at`.
- Plan bytes: look attributes, the `attr()` calls, and `set_looks` inputs are the same sequence as `HEAD~2`. The new data is sites-only. `c1` does not change lowering order or add plan constants.

## Findings

### 1. SHOULD-FIX — a look's class-versus-own flag uses a stale index

`contract/lower/src/states.rs:327` sets the flag with `i < class_len`. `class_len` is fixed at `contract/lower/src/nodes.rs:89`, before `compose_animation` (`nodes.rs:122`) removes animation longhands and before a grouped list prepends `list_rows` (`nodes.rs:180`). `fold_states` runs after both (`nodes.rs:338`). The look loop then trusts that flag (`nodes.rs:769`).

The base path does not. For a written row it uses `class_spans` (`nodes.rs:531`), which is why a comment there says those rewrites move rows across `class_len`.

Scenario A. A class has animation longhands, and the node writes the hover:

```
style Card
  animation-name=fade
  animation-duration=200ms
component App
  view
    box class=Card
      :hover
        background-color="#ffffff"
```

`class_len` is 2. Composing the longhands leaves the hover at index 0, which is still `< 2`, so the map says `class:Card` at the node's own line. `stateRung` walks up, hits `component`, and returns null (`tamber-inspect.mjs:213`). The row is refused as a style row the map gives no place for (`tamber-inspect.mjs:356`). The plan look is still the node's `#ffffff`. The edit is what breaks.

Scenario B, the other way. A grouped list's class writes `:hover` in its last rows. `list_rows` prepends two attributes, the hover index moves past `class_len`, and the map says `own` at the style's line. The join offers a template `set-row` there. The bytes can still land on that line, but the origin and the rung are the node's, not `set-state` on the style.

The span is already the author's. Class versus own should be `class_spans.contains(span)`, the same test the base row uses.

### 2. SHOULD-FIX — every border side is addressed as `border-color`

`scripts/tamber-inspect.mjs:184` maps `border_color_top|right|bottom|left` to `border-color`. A state part has no `attr`, so `rowRung` / `stateRung` use that name (`tamber-inspect.mjs:246`).

A side written as itself is legal: `admitted` treats a side as `border-color` (`states.rs:84`), and `longhands` leaves it as that side (`states.rs:99`). The map span is the `border-top-color` attribute. `plainAt` then looks for `border-color=` on that column, fails, and the rung says `` `border-color` is written as an expression ``. The shorthand case is the one the tests edit (`border-color` split into four sides that share that span). A hover that writes only `border-top-color="#111"` is not editable.

### 3. NIT — a conditional class look records one style's place

A base row of `class:A/B` drops `at` when the label contains `/` (`nodes.rs:541`). A look does not: `nodes.rs:769` stores `Class(label, None)` and the map always emits the part span (`map.rs:319`), which is style A's span (`class.rs:57`). Differing literals never compile (`lower-state-literal`). Identical literals compile, the join refuses the edit (`tamber-inspect.mjs:197` and `:341`), and `uses` still names A's line only (`tamber_uses.rs:396`).
