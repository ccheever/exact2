# Code review: QUEUE's notes on the web bugs, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `a8b592830e340ee68325b52d2dcaaa0a3add72cb1be2aecf2535dec382ef4556`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY (Astra), NOT READY (Grok).
- **Disposition:** Grok's MAJOR (the stale lead-in) and two MINORs (the contract invocation, what AppKit's notes say) and Astra's NIT taken in the landing commit.

---

- **NIT — [QUEUE.md:906](/Users/admin/x2p2/apps/exact2-comp/QUEUE.md:906):** “the first three look like small bugs” is stale after removing two items and explaining that the third needs a ruling. Delete that clause.

No BLOCKER, MAJOR, or MINOR findings. The fixes, disabled-dim policy and implementations, dependency direction, and distributed JS refusals match the code. Executing the route helper confirmed the `.team`/`.zip` distinction; the existing Accept test passed.

The momentum note appropriately remains unverified: tests synthesize the notification. [Apple’s older release notes](https://developer.apple.com/library/archive/releasenotes/AppKit/RN-AppKitOlderNotes/) support waiting for scroll animation completion, but do not establish current trackpad behavior.

READY
