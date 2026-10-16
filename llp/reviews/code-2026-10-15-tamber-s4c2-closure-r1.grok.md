# Code review: stage 4 commit 2: live or rebuild by the closure, round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `fbc5d771842e4c59707135c10ae08a6cc0e03112033b232200f7a269df83126a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both: failed builds cached as no synthetic; tokens that come to fold missed).
- **Disposition:** fixed in the first-round commit

---

I'll review the HEAD commit read-only against the stage 4 brief, starting with the diff and the surrounding fold, role, and synthetic-token code.The HEAD commit is the stage 4 live-or-rebuild change. I'll read the full diff and the compiler's fold path so the comparison is against the same rules.The diff is in. Next I’ll compare `folded` with the compiler’s synthetic-token rules and trace how role overrides and the server cache interact.The live-or-rebuild decision is wrong for a design edit that makes the compiler start keeping a synthetic token. That edit is classified `live`, and the answer leaves the synthetic out.

[P1] A design edit that makes the compiler newly keep a synthetic is `live` and omitted from the answer — `contract/cli/src/tamber_serve.rs:332`

`synthetic()` rebuilds its list only when a non-`.tamber` source digest changes, and `compile_path` failure is stored as an empty list. The compiler's kept set also depends on the design: a loose expression that does not fold is dropped (`contract/lower/src/theme.rs:174`), and a strict one fails the build, so neither appears in `plan.theme`. `plan` only re-folds the cached list. Setting `brand` from `AccentColor` to `#ff0000` while a row or state holds `color-mix(in srgb, brand 60%, white)` makes the compiler keep that mix, but the cache still has no expression. The token is not a role and the shape is unchanged, so the answer is `live` with `synthetic: []`. The running build either lacks that theme entry or is still the failed build; the batch cannot add it.

[P1] `lost` ignores a synthetic that fails before and folds after — `tamber/src/session.rs:215`

`lost` is set only when the after side reports a synthetic index the before side did not. The index is then removed from both sides (`tamber/src/session.rs:233`) so the folds stay comparable. After a committed edit to `AccentColor` (the case the new test calls rebuild), the cache still holds the mix, because the design is not part of the key. The following edit back to a hex colour fails on the before side and folds on the after side, so `lost` stays false, the mix is dropped, and the answer is `live` with no synthetic value. The build would keep the mix again.

The role check holds. A role override's folded value changes when any token it reaches changes, including through a chain, so that token is in `changed_tokens` and `Checked.role` is `Some` on that side (`tamber/src/session.rs:119`). A token becoming or ceasing to be a role changes `shape`. A `live` answer does not include a role token. `folded` uses `contract_lower::tamber_vocab()`, the same vocabulary as the compiler, and appends `~i` the same way. The compiler folds one design file, and so does `folded`; a synthetic cannot name a token that exists only in another `.tamber` file on either path. Names in `plan.theme` that contain `(` are the synthetic expressions. A kept mix that stops folding is rebuild, which is the right class for both a strict refusal and a loose entry disappearing.

The tests cover that stop-folding direction, a one-step role chain, and an added token. They do not cover a synthetic that starts folding, nor the edit that restores a colour after `AccentColor`. Every `propose` still builds the full contract source graph to form the cache key (`contract/cli/src/tamber_serve.rs:329`), including when the cached list is reused. That cost is real and is not what mis-classifies the edit.
