# Code review: stage 13 B, the pen importer, round 3's re-check 4 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `a1e5b20106`.
- **Method:** one brief (sha256 `5b011b879d860ac9c7f386f2901775de2f058f206eeaea36897984df9653f565`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- `tamber/src/import/dtcg.rs:987` — A colour alias that fails (chain longer than 256, or a cycle) is memoized only for the path `seen` was entered on (`dtcg.rs:1252` calls it once per mode tuple, each with its own memo). Every other colour walks up to 256 links again, and nothing in that walk spends. About 20,000 colour variables fit in a 1 MiB `.pen`, and six binary theme axes are 64 tuples (`dtcg.rs:1123` allows that), on the order of 3×10^8 recursive steps. Name charging for that file is a few hundred thousand units against the import budget (`3 << 24`). Number aliases do spend, in `raw` (`tamber/src/import/dtcg/kinds.rs:72`). The import runs in the debug isolate, which the watchdog kills at 10 s (`scripts/tamber-isolate.mjs:21`, `scripts/tamber-isolate.mjs:27`). The existing 18,000-link test stays fast because a resolved colour is stored for every link of that walk, and those names sort toward the resolved end.

**Should-fix**

None.

**Nits**

None.
