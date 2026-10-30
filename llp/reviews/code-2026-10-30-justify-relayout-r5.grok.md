# Code review: web hosts follow a parent's justify-items (canvas adoption, nesting), narrow re-check 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `/tmp/dt3-rv-c5`.
- **Method:** one brief (sha256 `f4bf29c0327b50a2bfeb502910be7adc7a1bcd83d508684ef50879fc2d9432e3`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll re-check only that fix diff, read-only, against the two prior majors: adoption of canvas `justify-self` and name alternation down a sized chain.The diff is three commits plus the red tests. Next I’ll read the sizing, writer, and adoption paths in full so the chain and ordering claims can be checked against the code.The two fixes hold. No blocker, major, or minor defect.

**NIT** — `host/web-js/src/sized.rs:9`  
The comment says a component root placed directly inside a canvas reads the first name because its parent is in another template. Uses are inlined into the one plan tree before `project` builds `parents`, so that root’s parent index is the canvas and it reads whichever pair that canvas writes.

Alternation matches on every chain checked. A sized node writes the `-2` pair only when its parent does not (`alternates`); the child reads `alt` of its parent, and `parent_css` writes that same bit. `box > canvas > canvas > canvas`, `canvas > progress`, and a progress with no sized child all line up, including a progress that still needs both the display and the alignment properties. A node never both writes and reads `-2`, so the two names do not collide down the chain. Custom properties are set on the parent element, so a grandchild sees its parent’s value rather than an ancestor’s.

`writes` picks those same names from the static CSS. `--exact-progress-justify:` does not match `--exact-progress-justify-2:` (the next character is `-`, not `:`). A `var()` uses commas (`var(--exact-progress-justify-2,start)`), so it is not mistaken for a declaration. A bound `display` or `justify-items` on an alternating parent therefore targets the `-2` pair. A bound `position` sees `justify-self:var(--exact-progress-aligned-2` whenever `own_css` emitted it. That string is omitted only for a canvas whose static position is absolute, and a bound position is not applied to the static style, so the `var()` is present and the writer uses the `-2` name.

`cv()` cannot drop a bound `justify-self`. `element_extras` emits it before the style rows, and `effect` runs the writer immediately after that, so the server inline `justify-self` is removed and the bound value is written afterward. Later updates re-run the writer only. Every canvas goes through `cv` (`parts.tag == "canvas"`); the removal is on the wrapper `div`, which is where the server inline style sits. The move preserves the old maps, the absolute-canvas early return, and `justify-self:auto` for an absolutely positioned progress.

VERDICT: Both majors are fixed; alternation, `writes`, and `cv()` ordering hold for the chains above.
