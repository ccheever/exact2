# Code review: Stage 12 E: @exact/cards, @exact/actions and apps/library-gate (LLP 1139 §5 E), round 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `086a3086af`.
- **Method:** one brief (sha256 `994835612436bff863a06c3a227f9a3afd7084b7fdcfee882bc4ef613f768151`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking or should-fix; nit: the marker check is a loose substring
- **Disposition:** taken in 9a1fe59dcf (the structural marker check)

**Blocking:** none.

**Should-fix:** none.

**Nit:** `scripts/library-gate.test.mjs:72`. The marker check is `includes` of `box testId="marker-${id}" width=24 height=24`. A comment that contains that text passes, and so does a longer token with the same prefix (`height=240`, `height=24px`). The glyph half of the same line is an exact trimmed `text glyph`, and the placement parser just above it already requires a real one-line call. The fixture’s markers are real 24×24 boxes with `text glyph` inside them; only the check is loose.

`bun test scripts/library-gate.test.mjs` passed (2 tests). The compiler’s `library` IR matches the fixture: the four exports, one cell per D5 value, no `if`/`for`/`match` in the view, unique ids, and a marker on every PostCard, including the `sample-` cells.

**@exact/actions:** clean. Private package, entry re-exports only `PostButton` and `ActionRow`, design in `actions.tamber`. Spacing and corners are tokens or params (`post-pad` is `space.md`; `row-lead` / `row-lead-big` are the decided negative margins). The test’s compile accepts the buttons as authored pressables (`paint`/`text`/`shape`/`space`, `:disabled` paint, host-state token literals). The cells the fixture treats as variants (`big`, `haptics`, `reposted`, `liked`) are the ones the compiler reports; `saved` and `replyDisabled` stay `sample-` cells.

**@exact/cards:** clean. Same D0 shape for `PostCard` and `AtCard`. Knobs name tokens or params; `context-hang` and `badge-hang` are the other two negative margins, both `margin-left`. `PostCard` places `children` once, on part `content`, and the slot check passes with `lineAbove`’s zero border and `min-width=0`. `context` stays structural (`contextText` is the app’s words on both kinds). `AtCard` has no D5 props, so one cell is the whole requirement.

**library-gate:** meets §5 E, with the nit above. Placements are the D5 matrices plus the decided `sample-` states, sample props are fixed, part `testId`s are stamped from `id` inside the packages, and `marker-toggle` switches `glyph` between `A` and `B`. `scripts/async.mjs` schedules the test on the glue lane.
DONE
