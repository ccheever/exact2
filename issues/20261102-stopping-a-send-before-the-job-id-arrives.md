# Stopping a send before the job id arrives leaves the reply running

**Status:** Closed
**Systems:** Messages (`apps/messages/app.ts`), Agents (`apps/agents/app.ts`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** Stop during the accept of `POST /turns` does not abort the job. In Messages the outgoing text stays, with no reply and no error, and Retry starts another job. Each orphan counts toward the service's six-job cap until the job finishes or its own abort fires, so repeated stops answer "Six models are already responding." In Agents the blank assistant row stays in the thread and the service keeps generating.

**Cause.** Messages `start` sets `pending` and then awaits `POST /turns`. Stop clears `pending` and calls `DELETE` only when `t.job` is already set (`command`, the `stop` arm). When the accept returns, `start` stores `t.job` without reading `pending`. `pulse` follows threads that are both pending and have a job, so the late id is never polled and never aborted. Agents `start` pushes an empty assistant row and sets `streaming` before the same await. Stop clears `streaming` only while `c.job` is empty, and the accept then assigns `c.job` anyway (`apps/agents/app.ts`, `start` and the `stop` arm).

**Reproduction.** Send and stop before `POST /turns` returns. In Messages, no `DELETE` is issued and a later pulse does not mention the job. In Agents, the empty assistant row is still in the conversation after the accept.

**Fix.** After `POST /turns` resolves, if the user already stopped, `DELETE` that job and do not keep it. In Agents, drop the empty assistant row when the turn was stopped during the accept. Treat stop as a generation checked on both sides of the await.

**Acceptance.** Stop during the accept aborts the job the accept returns, and a following send is not refused for the six-job cap. The Messages thread shows the sent text with no stuck pending state. The Agents thread does not keep the blank row. Stop after the id has arrived still deletes that job, as it does now.

**Resolution.** Track the identity of each send through acceptance and polling. Stopping invalidates that turn immediately; a late job id is deleted and cannot revive or modify a newer send. The Agents app also checks identity after on-device awaits.

**Verification after the fix.** Messages and Agents Bun regressions passed stop-before-acceptance, late job cancellation, and a new send remaining independent of the stopped turn.
