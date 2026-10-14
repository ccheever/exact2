# Ibex consumer adoption — Codex implementation review

- Date: 2026-10-13
- Reviewer: `gpt-5.6-sol`, reasoning `xhigh`, independent collaboration review
- Revision: `3ad9ef9cb..214a0250f`
- Scope: source review; no edits, builds, delegation or remote commands
- Verdict: **one material finding**

The portable worker adapter retained process-wide worker slots in a shared
budget owned by its service and dispatched jobs. Dropping an idle service
closed the queue and released that budget before detached worker threads had
left their receive loop. Runtime replacement could therefore admit new
workers while old workers were still retiring. Partial construction failure
followed by retry had the same gap.

The review requested that each actual worker own its slot through exit, or
that retirement otherwise confirm exit before returning capacity. The
implementation response is the upstream per-thread guard constructor and
Exact's transfer of individual slots into it, committed in `b51bfce56`.

The remainder of the review found no material defect in retained document
authority, logical-path errors, result limits, image cancellation/publication,
transport admission rollback, activation deadlines, redirects or portable
upload accounting. Final revision review is recorded separately.
