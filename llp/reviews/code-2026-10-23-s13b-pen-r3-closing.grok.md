# Code review: stage 13 B, the pen importer, round 3's closing re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e0bdc24d42`.
- **Method:** one brief (sha256 `abe7f256b2324332943691f30057d5a5e5ba523398fd4009c593320fa097c670`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

None. A laid colour, family, or string is at most 1024 bytes (`pen.rs:674` stores `Value::Null` and a refusal), so `laid`, `reads`/`text`, and `marks_of` never see the old long literal. After `spent` is set, later diagnostics break (`landing.rs:377`) and `clears` returns before `skip.clone()` (`landing.rs:436`).

**Should-fix**

- `tamber/src/import/landing.rs:249` — A when-line’s condition strings are copied in full (`conds.to_vec()` in `laid`), but the charge adds `16 * c.len()`: one unit per condition, not per 16 bytes of those strings. Axis names and values may be 64 characters each, so a `.pen` of long theme names is undercharged on every probe (about 8× for ASCII, more for non-ASCII). Name, default, and when-value bytes are charged.
- `tamber/tests/it/import_pen.rs:1071` — `all(...)` on the `g*` filter is true when that filter is empty. If the expressions were mapped and folded, this assertion would still pass. Require those rows to be present and to say `no number`.

**Nits**

- `tamber/src/import/dtcg/pen.rs:668` — A literal in braces is handled before the 1024 cap and kept with `value.clone()`. It is refused, so it is not laid or marked, but the long string is still stored and copied into the report.
