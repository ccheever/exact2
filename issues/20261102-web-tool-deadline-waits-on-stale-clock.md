# A web tool call's deadline does not advance while its request waits

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, tool calls, WebMCP
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, a tool call waiting on I/O can remain running past its ten-second limit when the page has no authored timer and no new commits. Its caller and later queued calls keep waiting until some unrelated work moves the page clock.

`host/web-js/calls.js:131-149` compares deadlines with `clock.now` and polls every 20 ms, but never refreshes that clock. In a live page `rt.js:287` updates it only when `time()` runs; the runtime's driver at lines 345-352 schedules no wake without an authored task or due mutation. An outstanding request alone does not continuously advance it. The five-second wait behind a wake has the same stale-clock check.

Run `bun /tmp/exact-review-tool-timeout.mjs`. It copies the actual `calls.js` into a private temporary directory with a minimal runtime clock and pending-ticket table, dispatches a call that owns an outstanding ticket, and waits 10.2 seconds. The call still answers `state=running` at clock 0. Moving the clock to 10001 and invoking the normal after-commit hook immediately produces `outcome=timeout`. This exercises the call scheduler; a compiled app, WebMCP browser API and real network request were not exercised. Runtime clock scheduling was checked in source.

Refresh live time before checking deadlines and arm a runtime wake for the nearest call deadline, preserving the agent's explicitly driven clock. Check an idle live page whose tool starts a request held beyond ten seconds, and a call queued behind a wake.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

## Resolution

Refresh live time when calls arrive and while pending, and schedule a runtime clock wake for queue and execution deadlines. Keep agent time explicit.

## Verification

`bun test host/web-js/scheduler.test.mjs` passes execution timeout and queued-call timeout without unrelated commits, plus explicit agent-clock advancement.
