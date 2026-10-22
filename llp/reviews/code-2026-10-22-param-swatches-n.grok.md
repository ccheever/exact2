# Code review: the design tool's colour param swatches and chain words, a narrow check of the post-round fixes (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** a narrow check of the post-round fixes; HEAD `ef43f61b37` (shas before the rebase onto main). The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; round 2 closed.

I'll verify those three Round 2 findings against HEAD only—read-only, no tree changes, builds, or tests.HEAD drops role colours, shows swatch text, and cites a value only when tuples agree. I'll read `lookOf` and the Badge.well path to see whether that holds.All three findings are closed at `ef43f61`. Nothing blocks.

1. **System colours.** Swatch css goes through `isRole` (`components.ts:195`), so both an `-exact-` role and a CSS system colour such as `Canvas` are dropped. An unset role still reads "the platform's role" (`components.ts:201-202`). An app-set line still reads `set for this app:` plus its value; if that folded css is the keyword, its swatch is dropped too.

2. **Visible swatch text.** Each swatch is a row: the square, then `<column label> <value>` (`app.contract:2477-2479`).

3. **Mode-varying token.** With no dials, the chain cites the stored value only when every tuple's value is the same; otherwise it says `its value by mode, in its row` (`components.ts:197-198`). `surface.inset` has no dials and two values (`light-dark(#f8f9faff, #101214ff)` and `light-dark(#eef0f2ff, #0c0e10ff)`), so Badge.well is `surface.inset → its value by mode, in its row` (`app.test.contract:2143`). A dial (`accent.fill → accent`) and a shared value (`-exact-label → light-dark(…)`) still take those paths.
