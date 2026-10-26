# Code review: the top-layer layout-position accessor, its guard and its audit table (theme2 10:42 (A)), narrow re-check 4, 2026-10-22 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `d6ac43e287`.
- **Target:** d6ac43e287.
- **Method:** one brief (sha256 `3c620fc1ad07d5986d2400489ca93b6509631cac1a9a55e1f8d5e81f9be7e5cd`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 6bf02edd22 (the row id only written out; renames, aliases, groups refused; interpolation comments)

VERDICT: WRONG

1. **blocker** — A renamed group import still hides the row-id read. `scripts/position-reads.test.mjs:170` drops every bare `PositionType` between `use ` and `;`, and `PositionType as P` is not an alias of `StyleId`, so this flags nothing:

```rust
use crate::StyleId::{PositionType as P};
fn f(s: &Row) { s.get(P); }
```

That rename is the ordinary way to bring the row id in beside the `PositionType` enum. The un-renamed `s.get(PositionType)` after `use StyleId::{…, PositionType}` is flagged.

2. **should-fix** — A bare row id in a struct literal is treated as a type. `scripts/position-reads.test.mjs:28` the lookbehind `(?<!(?:::|->|:)\s*)` rejects `Foo { id: PositionType }`. In Rust that colon introduces a value. Call, array, match, and `return` forms are flagged.

3. **should-fix** — The alias ban only matches `StyleId as`. `scripts/position-reads.test.mjs:30` misses both of these, and `Ids::PositionType` / `Id::PositionType` then match nothing:

```rust
use crate::StyleId::{self as Ids};
type Id = StyleId;
```

`use crate::StyleId as S` is flagged, which is the form the scratch test plants.

4. **should-fix** — Interpolation still ends on a paren inside a comment. `scripts/position-reads.test.mjs:134` counts `(` / `)` and nested strings only, so this valid Swift ends the string at the `"` of `"/*"` and the leftover `/*` blanks the later read:

```swift
let s = "\( /* ) */ "/*" )"
let y = view.style["position_type"]
```

`"\(String("/*"))"`, `#"x \#"# /* "#`, and `#"hello \#(user ?? "friend")"#` stay inside the string, so the read after them is visible.

5. **nit** — `<StyleId>::PositionType` is not a `StyleId::PositionType` match (`scripts/position-reads.test.mjs:28`); the `>` sits between the name and `::`. A path import `use crate::StyleId::PositionType;` is flagged even though group imports are skipped (`:170` only filters a match whose text is exactly `PositionType`).

The scanned tree reports 14 reads, each allowlist entry once, and no stray. `kernel/src/kernel/cover.rs:349` is the `if [Display, PositionType, OverflowX, OverflowY]` use; the import above it is not a hit. The scratch fixture's line lists match the test (`2,3,6,9,10,22,23,24,26,27,31,33` and `1,4,5,9,10,11,12`). The audit rows match `ab4c231328`'s five removed allowlist lines — `refresh_sticky`'s field and its row id, `sticky_constraint`, Apple's crossing rows, Linux's `sticky()` — plus this commit's move of the `cover.rs` row from the import to that use. `bun test` could not run here (it writes a temp tree); those checks used the same `code` / `source` / `directReads` functions in memory.
