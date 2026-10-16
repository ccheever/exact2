# Code review: LLP 1121 stage 3, commit 6, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `17b5bf6126a446f26db2cbd9b8b48bf81e7471b769bcdee4dd07bdb47229e4db`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, nine medium, one low; Grok: one high, five medium, four low.
- **Disposition:** fixed in round 2; an unclosed rule's later rules stay nested in it, as CSS nesting reads them

---

## High

### 1. Re-import unsets `when` lines on axes this importer does not own
`tamber/src/import/mod.rs:148`

`reconcile` treats every `when` on a mapped token that the new import did not emit as the source's to take back. shadcn only ever emits a single `theme=<value>` condition. A hand-written `when platform=ios` or `when theme=blue platform=ios` on `shadcn.background` becomes `unset-token-when` inside the token transaction, which is the part accepted when the import is taken. An unchanged stylesheet then proposes that unset, so the "proposes nothing" path only holds when the token has no extra conditions.

Fix: unset a `when` only when every condition is the importer's own axis. For shadcn that is a single `theme` condition. Leave any line that names another axis.

## Medium

### 2. A later non-colour does not win, and a selector list keeps the stale colour
`tamber/src/import/shadcn.rs:296`

Supersession runs only after a declaration has already been accepted as a colour, and only by writing into `sides`. A later length or other non-colour is reported and does not clear the earlier colour. With a selector list the earlier declaration is still live on another target, so it stays mapped and the overridden target keeps the old colour. The count still balances, because the later line is the reported one.

```css
:root, .dark { --a: red; }
.dark { --a: 1rem; }
```

`--a` becomes `red` in both schemes. `.dark`'s `1rem` is only in the unmapped report.

Fix: pick the last declaration per `(target, variable)` before classifying it. A non-colour winner clears that target. A declaration that loses every target is reported as superseded and counted there. A selector-list declaration that still wins another target stays mapped, and the lost target must not keep its value.

### 3. Colour-hood is a file-global fixed point
`tamber/src/import/shadcn.rs:245`

A variable is a colour if any declaration is a literal colour, or a `var()` to such a variable. Each `var()` is then mapped in every target. A name that is a colour in `:root` and a length in `.dark` still makes `.dark { --alias: var(--ink) }` a colour reference, including when one declaration's selector list covers both targets. That declaration is one source value, so the count cannot both map it and report it.

Fix: decide colour-hood per target, from the winning declaration in that target (finding 2). Write `sides` only where the value is a colour there. Count the declaration mapped when any of its targets is. Where a `var()` is not a colour, leave that target empty. The referenced declaration is the one the report already carries.

### 4. An unclosed `{` or a stray `}` drops the rest of the file
`tamber/src/import/mod.rs:323` and `tamber/src/import/mod.rs:332`

The parent slice already excludes a block's matching `}`, so a `}` inside `body` is stray. The reader `break`s, then parses from `start` to the end of the text as a single declaration. That segment starts at a prelude or a `}`, so the `--` names in it are discarded. An unclosed `{` takes the same path and never reads the block it opened. `source` only counts declarations the reader returned, so the count still matches and the import succeeds short of those colours.

Fix: on a stray `}`, set `start` past it and keep scanning. On an unclosed `{`, parse the remainder as that block's body.

### 5. Nested rules are classified from the innermost selector, and `&` is ignored
`tamber/src/import/shadcn.rs:186`

`css_blocks` keeps the full context. The importer then uses only the innermost prelude that is not an at-rule. `&` is an unknown simple selector, so it is skipped. Two shapes that are normal nesting land on the wrong side:

- `.theme-blue { &.dark { --primary: #008; } }` becomes the default dark value. The parent theme is discarded.
- `.dark { .theme-blue { --primary: #008; } }` becomes theme blue's light value. The parent `.dark` is discarded.

A bare `& { }` block is reported as "not a theme block", so that one is visible. The two above are mapped to the wrong target.

Fix: walk every non-at-rule prelude in the context, substitute `&` with the parent selector, and classify the combined selector. When `&` has no parent, report the declaration.

### 6. A role offer does not clear `when` lines it treats as not-yet-set
`tamber/src/import/shadcn.rs:437`

The offer is skipped only when the role's value is already the token and `whens` is empty. If the role has any `when`, the offer is a `set-token` of the default value alone. Accepting it leaves the `when` lines, so under those conditions the role still is not the imported token, and the next import offers the same op again.

Fix: pair that `set-token` with an `unset-token-when` for each existing `when` on the role. After it is accepted, the value matches and `whens` is empty, and the offer stays off.

## Low

### 7. A theme whose declarations are all reported never becomes an axis value
`tamber/src/import/shadcn.rs:298`

Theme names are recorded only while a colour is written into `sides`. `.theme-blue { --radius: 0.5rem; }` is reported as a length and `theme` is not proposed, so the source's theme class is absent from the axis. The declarations themselves are in the unmapped report.

Fix: record a theme from every declaration whose selector resolves, and emit the axis value even when no colour `when` uses it yet.

### 8. One bad alternative fails the whole selector list
`tamber/src/import/shadcn.rs:37`

`target`'s error is propagated with `?`. `:root, .theme-2 { --a: red }` is reported entirely as "the theme `2` is no axis value", and the `:root` colour is not imported. Unknown simple selectors inside one compound are ignored. An identifier failure is not.

Fix: skip the bad alternative, keep the ones that resolved, and report the declaration only when every alternative failed.

### 9. `review.tokens` stays at the original count after a partial accept
`scripts/tamber-server.mjs:418`

Offer `items` are remapped onto the kept operations. `tokens`, the index where the role ops begin, is copied unchanged. The tested path keeps every token op and drops the offers, so `23` is still the boundary. Keeping a subset of the token ops, or a token op plus an offer, leaves `tokens` larger than the new list, and the offer boundary moves.

Fix: set `tokens` to how many kept indices were below the original `tokens` value.

### 10. A new theme axis can refuse the whole proposal at the tuple cap
`tamber/src/import/shadcn.rs:385`, cap in `tamber/src/check.rs:305`

Each theme value multiplies the design's tuples, and 64 is the ceiling. The axis is proposed with the colour tokens in one transaction. A design already near the cap fails `check` on `propose`, so none of the colours are held. The importer does not surface that as an unmapped theme.

Fix: when the axis would push the product over 64, report the theme classes and propose the tokens without `add-axis` / `add-axis-value`, or split them so the colours can still be accepted.

Theme-only defaults match the comment and `shadcn_reads_css_as_css`: the first theme's resolved value becomes the token default, and a `when` equal to that default is dropped. I did not find a further defect in that rule.
