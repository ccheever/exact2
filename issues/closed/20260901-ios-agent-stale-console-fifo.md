# iOS agent leaves a stale launch FIFO

**Status:** Closed
**Resolution:** The iOS agent now observes early launch exit and awaits console termination before deleting its socket directory.
**Systems:** Agent API, Apple host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1012

Two of three consecutive iOS agent sessions failed before opening the socket:
`simctl launch --console` reported `Error 17: File exists` for its console FIFO.
A full `node scripts/smoke.mjs ios` rerun passed, making this a repeatable
session-cleanup race rather than an app refusal.

`scripts/agent.mjs:322-328` closes the socket, waits at most one second for the
console process, sends `SIGKILL`, and immediately removes its temporary
directory without awaiting process exit. The next `--terminate-running-process
--console` launch can begin while the previous console attachment is still
being torn down.

After killing a recorded console PID, await its `exit` event before returning
from `close`; also include the launch stderr immediately rather than timing out
for 20 seconds. Add a loop that opens and closes at least three simulator
sessions against the same bundle.
