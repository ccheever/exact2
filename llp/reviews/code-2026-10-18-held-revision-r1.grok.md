# Code review: tamber/1.12, the long poll's held revision, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the held-revision commit.
- **Method:** one brief (sha256 `c5a1b805efad5ab370019951e00ac28e7bfccb968e43534acf480e17c7fed50b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (no findings; drop, expiry, eviction, recompute untested).
- **Disposition:** tests added in round 1's commit (drop, recompute).

---

Nothing blocking.

No findings. Every change to the held list bumps `heldRevision`, and the design tool only treats a missing `proposals` as "keep what I have" when it sent that same revision for this run's epoch.

What I checked:

- **Server mutations.** `forget` (drop, interval `expire`, cap eviction), `hold`'s `proposals.set`, `accept`'s delete, and `restore`'s `proposals.set` all increment `heldRevision`. `expire()` at the start of `hold` and `accept` goes through `forget`. The accept paths that send `proposal-dropped` without a further bump (restore-on-expiry, replan stale, replaced) run only after the proposal is already out of the map, so the list the client would keep is already without it.
- **Poll vs accept.** `answer()` reads `heldRevision` and `held()` in one turn. `send()` only schedules the poll reply as a microtask, so a reply woken by `publish` or `hold` sees the list after the rest of that turn (including `settled` / `replaced`). A poll that arrives during `accept`'s `await` can observe the proposal removed; `restore` or the replacement `hold` bumps again, so the next poll gets the list back.
- **Epoch.** Omission requires `request.held === heldRevision` and `request.epoch === epoch`. A restart, a missing `epoch`, or a missing `held` still sends `proposals`. The tool sends `held` only when `heldAt.epoch` is the epoch it is polling.
- **Tool.** The first poll has no epoch, so it does not send `held`. An error or a superseded poll (`gen !== polls` before the update) leaves `heldAt` and the list alone. `heldAt`, `heldRaw`, `heldToken`, `heldOps`, and `heldReview` update together, and only when `proposals` is present. `acceptHeld` copies ops and the proposer before its await; `acceptImport` and `editImport` read `heldOps` and `heldReview` before theirs. An empty `proposals` array is still an update. Swatches redraw when that array arrives or when `Math.max(...snapshots.keys())` changes; reads store a new key, they do not rewrite an old one.
- **Stub and subscribe.** The stub still returns `proposals` on every poll and ignores `held`, so the tool keeps replacing its list. `subscribed` still carries the full list and no revision; the long poll is what omits.
- **Version.** `PROTOCOL`, the protocol page, and the hello assertions are `tamber/1.12`. Nothing else compares `tamber/1.11` for equality. Old clients that do not send `held` still get `proposals`.

Residual risk: the new test covers an unchanged list, a wrong epoch, and an accept. It does not cover drop, expiry, cap eviction, or a delete-then-restore recompute. Those paths do bump in the code above.
