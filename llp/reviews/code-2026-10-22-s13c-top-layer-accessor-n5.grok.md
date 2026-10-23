# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 5, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6bf02edd22`.
- **Target:** 6bf02edd22.
- **Method:** one brief (sha256 `a4bac0f0766392c6f1dce1e26fd36780c8fd27fd99aa16278255f18639de586c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken in a946d58d4f (Self::, grouped globs, renames after a glob, re-exports, module-wide globs, split aliases, Swift """ keys)

VERDICT: RIGHT WITH NITS

The four forms that hid a read last round are caught, and the guard is green on the tree (`bun test ./scripts/position-reads.test.mjs`, 3 passed). `cover.rs` still checks the same four rows with the same `mask.has` loop.

1. **nit** — `scripts/position-reads.test.mjs:35` A type alias is refused only when it is `type Name =` plus a path of word characters and colons. `type Id<T> = StyleId`, `type Id<'a> = StyleId`, `type Id = crate :: StyleId`, and `type Id = crate::` split before `StyleId` all miss, and `Id::PositionType` is not a read. The one-line form `type Id = crate::StyleId` is refused.

2. **nit** — `scripts/position-reads.test.mjs:38` Bare `PositionType` is skipped whenever it sits in a path. `mod p { pub use crate::StyleId::*; }` then `s.get(p::PositionType)` is not a hit, and another file’s `use crate::p::*;` plus `s.get(PositionType)` is not a hit either, because that file never says `StyleId::*`. That is the same spelling as the enum path `crate::PositionType`, which has to stay quiet.

3. **nit** — `scripts/position-reads.test.mjs:25` `Self::PositionType` inside `impl StyleId` matches neither the written-out read nor a refused import.

4. **nit** — `scripts/position-reads.test.mjs:34` Any `as` in a `use` that mentions `StyleId` is refused and, at line 199, cannot be allowlisted. `use crate::StyleId::{Display as D}` fails the guard with no `PositionType` involved. Nothing in the tree does this today.

5. **nit** — `scripts/position-reads.test.mjs:189` The `StyleId::*` check is file-wide, and `source` (line 163) blanks only a `#[cfg(test)] mod`. A `#[cfg(test)] fn` that globs `StyleId` also flags a production `fn prod(p: PositionType)`. A glob inside a `#[cfg(test)] mod` does not.

Checked and holding: `use StyleId::{PositionType as P}`, `use StyleId::{self as Ids}`, `type Id = crate::StyleId`, a group import (including `Req { id: PositionType }` after it), a glob plus `s.get(PositionType)` and `Req { id: PositionType }`, and `crate::PositionType` / `PositionType::Static` left alone. The Swift scanner keeps the next `"position_type"` after `"\(/* ) */ "/*")"`, and the same for a `//` comment, a raw `\#(…)`, a quote inside the comment, and a nested block comment. Field reads and destructuring still count. `use crate::StyleId::PositionType` (no braces) fails the guard on that line; the later bare calls are not separate hits, so allowlisting the import would leave them unseen.
