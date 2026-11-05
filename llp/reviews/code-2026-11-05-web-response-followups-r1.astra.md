# Code review: web response follow-ups, round 1, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `84eafb1a03` on `ea352093a8`.
- **Method:** one brief (sha256 `3b73b86de91e104041b76818d62a93cb387016c67fad12a5b9e46e04ed88d190`), one reviewer, pointed at the commit, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree’s paths.

**One finding: P2 — SSR now trusts a pending signal that rollback does not restore.**

The new selector in [build.mjs:580](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-response-review-wsqat4rr/checkout-r1/host/web-js/build.mjs:580) reads `r.pending()`. Resource settlement changes that signal, but [rt.js:533](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-response-review-wsqat4rr/checkout-r1/host/web-js/rt.js:533) saves and restores the resource’s value and ticket without restoring pending.

**Trigger:** an asynchronous answer changes dependent resources’ arguments; one dependent resource answers or starts a request, then a later source refuses the settlement. Source refusals legitimately reach this path through [rust-data.js:95](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-response-review-wsqat4rr/checkout-r1/host/web-js/rust-data.js:95).

I reproduced both directions of the inconsistency:

- An answered resource tentatively starts another request. Rollback discards that request but leaves pending **true**. No request starts and no work remains outstanding, yet the candidate returns **503/unsettled**; the base returns the retained head’s **410/settled**.
- An unanswered long poll has received only a heartbeat. Its tentative reask returns synchronously, clearing pending. Rollback restores the original unanswered stream but leaves pending **false**. The candidate returns **200/settled**; the base correctly returns **503/unsettled**.

This is an **introduced HTTP regression exposing an existing rollback defect**. It violates the requested preservation of unanswered-stream pending semantics.

**Proposed settlement:** snapshot and restore the pending signal alongside the resource’s value and ticket. Add renderer regressions for both refused-settlement cases. An in-memory restoration control corrected both outcomes in my probes.

The Retry-After implementation is accepted. The normal stream settlement and resubscription paths also passed my probes, but the stream change is not yet fully accepted because of this finding. There are no prior-round settlements on this topic. All four review passes are complete; I found no separate simplification or writing issue.

I ran read-only probes using the actual runtime, stream reader, renderer composition and public serve adapters, with controlled I/O and filesystem inputs. They covered heartbeats, first and later events, client resubscription, GET/HEAD, compression, and independent negative controls. I did not rebuild compiled fixtures, launch browsers or run the required gates. Supplied logs report passing focused suites and the two caps violations outside this diff; Cargo/boot/workspace outcomes remain unverified. Diff checks passed, and no files were changed.