# Ibex consumer adoption — Grok worker lifetime follow-up

- Date: 2026-10-13
- Reviewer: requested `grok-4.6`, reasoning `xhigh`; CLI reported `grok-4.6-build`
- Session: `01a13487-d99b-7c51-9698-cd6fcf44e170`
- Revision: `214a0250f..b51bfce56`, plus upstream `30b0db0..de11540`
- Scope: immutable local source packet; no edits, builds, delegation or remote commands
- Verdict: **no material findings for either delta**

The reviewer confirmed that each upstream pool thread holds its guard through
receive-loop exit and that partial construction releases only unstarted guards.
Exact transfers individual reserved slots out of the shared budget before
construction. Each actual worker then owns its process charge, including idle
retirement and blocked work. Failed lane initialization is cached permanently,
so a retry cannot reuse slots retained by previously started threads.

The review also checked ordering: base startup publishes its budget under the
core lock before admitted work can ask a lane for a worker. It inspected the
isolated consumer accounting fixture and both upstream guard witnesses.

No other reviewer findings were supplied. The subsequent base-startup failure
correction at `19aa3f888` was not part of this snapshot and receives a final
delta review recorded in the local delivery ledger.
