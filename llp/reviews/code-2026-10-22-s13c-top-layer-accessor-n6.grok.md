# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 6, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `a946d58d4f`.
- **Target:** a946d58d4f.
- **Method:** one brief (sha256 `79cf239a4673d8762791d0c5b518109ef575a4da63d8619a0c837b15842b13c3`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 32d361f7eb (a glob in any module's scope); theme2 12:59: the bar is met once this round is in, an ordinary form found added with its test and Grok noting the fix

VERDICT: WRONG

1. **should-fix** — A module-level `StyleId` glob is refused only when `use` starts the line (`scripts/position-reads.test.mjs:37`). An inline `mod` indents that `use`, and the glob still puts the row id in the module namespace. The scanner reports no read for these, each run through `directReads`:
   - `pub mod rows { use crate::StyleId::*; pub mod child; }` with a child `use super::*; s.get(PositionType)`
   - `pub mod rows { use crate::StyleId::{self, *}; … self::PositionType }`
   - `pub mod rows { use crate::StyleId::*; pub use PositionType; }` with `rows::PositionType` in the same file, and with another file's `use crate::rows::PositionType; s.get(PositionType)`
   `pub use PositionType` also misses the re-export pattern at line 38, which requires that `pub use` to spell `StyleId::`. A path-qualified `PositionType` (`self::`, `super::`, `rows::`) is skipped by the bare-name check at line 41, so once the glob line is skipped the read is skipped with it. A column-0 `use crate::StyleId::*;` is refused, and a function-local glob still flags a bare `PositionType` in that file. The scratch tree only plants the column-0 parent (`parent.rs:273`).

2. **nit** — The type-alias pattern (`scripts/position-reads.test.mjs:35`) misses `type Id<T> where T: Clone = crate::StyleId` and `type Id<T: Foo<Bar>> = crate::StyleId`. Both scanned clean. `type Id<T> = crate::StyleId` and a path split across lines are refused.

3. **nit** — `<StyleId>::PositionType` and `<Self>::PositionType` are row-id reads, and the read pattern at `scripts/position-reads.test.mjs:25` leaves both unseen. `Self::PositionType`, including a newline after `Self::`, is reported.

Same-file `{self, *}` plus a bare `PositionType`, `use PositionType as …`, a `pub use` that spells `StyleId::`, a split or generic alias, and the Swift key forms (`"""`, indented `"""`, `#"…"#`, `#"""…"""#`) are all reported. A comment inside a Swift interpolation still leaves the following `"position_type"` visible. `cover.rs` `style_changed` still checks `Display`, `PositionType`, `OverflowX`, and `OverflowY` with `mask.has`. `bun test ./scripts/position-reads.test.mjs` — 3 pass, so the tree is not refused.
