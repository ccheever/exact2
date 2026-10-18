# Code review: tamber/1.12, the long poll's held revision, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the held-revision commit and round 1.
- **Method:** one brief (sha256 `d724b20c5b46392832579dd0885ed5af60461de92bc4e59c9cb42e8f436d99b6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (no findings).
- **Disposition:** fixed in round 2's commit.

---

Nothing is blocking. No findings.

The held revision is a monotonic counter bumped in the same synchronous step as every list change (`hold`, `drop`, `accept`'s delete, `restore`, expiry, and cap eviction through `forget`). A poll omits `proposals` only when `request.held === heldRevision` and `request.epoch` is this run's epoch, so a stale revision from the same epoch still gets the list. The design tool stores that revision only from a response that included `proposals`, and it sends the revision only when its stored epoch matches the epoch it is polling.

Checked the cases round 1 named:

- Renaming the client (`polled`) keeps the same server list. The new poll still names the current revision, and a superseded poll returns before it can overwrite `heldAt`.
- A new run fails the epoch check, so the answer includes `proposals`. The tool installs that list before `forget`, and `forget` does not clear it.
- `missed: true` with the same epoch still runs the revision check. A gap that did not change the list omits it; a gap that did sends it, and the tool applies that list.
- No other client of `POST /__tamber/events` sends `held` or requires `proposals`. Agent scripts do not poll it. The stub always sends `proposals` and ignores `held`, so the tool keeps working against it. Subscribe is unchanged.
- The answer's `files` map is path to digest, not file text. Omitting an unchanged held list is what removes the large body.
- The version note matches the code: `held` on each answer, and `proposals` left out only when the request names the current revision for this run.
