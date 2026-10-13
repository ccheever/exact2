# Ibex consumer adoption — Codex worker lifetime follow-up

- Date: 2026-10-13
- Reviewer: `gpt-5.6-sol`, reasoning `xhigh`, independent collaboration review
- Revision: `214a0250f..b51bfce56`, incorporating the prior consumer review
- Scope: source review; no edits, builds, delegation or remote commands
- Verdict: **one material startup-failure finding remains**

The per-thread guard transfer correctly fixes the original pool retirement
gap. Individual slots remain owned by actual pool threads, including after a
partial pool construction failure; failed initialization is cached.

The reviewer found an earlier startup path still retaining unused slots:
`Machine::start` stored the remaining seven lazy-pool slots even when one of
the six base-thread spawns failed. The caller then disabled the core, but a
retained failed core could keep those seven slots indefinitely without any
corresponding pool threads. Repeated failures could deny unrelated cores
process-wide admission.

The requested correction is to publish unused slots to the lazy pool budget
only after successful base startup; on failure, unassigned slots must drop.
No other material finding was reported. The correction and its fault-injection
witness require review on their own revision.
