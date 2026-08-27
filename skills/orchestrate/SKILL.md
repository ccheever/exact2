---
name: orchestrate
description: Run a multi-lane program to completion across the machine fleet — route coding to codex, reviews to two blind model families, supervise on a 20-minute judgment tick, and report milestone/percent/ETA with a program total. Use when the user hands over a body of work and wants it driven rather than discussed.
---

# orchestrate

Drive a body of work to done. You own sequencing, dispatch, and judgment. The user owns
scope and the decisions you are not confident about.

## Trigger

"Orchestrate this," "supervise this work," "run this program," or any handoff of a
multi-part body of work where the user expects progress rather than a plan.

## Posture

Decide autonomously and keep moving. The user is not a queue you block on. When you hit a
fork you can resolve from the code, the brief, or a sensible default, resolve it and log
it. Blocking questions are for decisions where proceeding under any assumption would be
unsafe or would waste the work if wrong.

## Model routing

| Work | Model |
|---|---|
| Implementation | codex `gpt-5.6-sol` — pick high / xhigh / ultra per task |
| Code review | **both** grok `4.6` **and** codex `gpt-5.6-sol xhigh` |
| Everything else | your call |

Reviews are **mutually blind**: two families, neither shown the other's findings, both on
the same revision. Agreement across families is signal; a finding only one family raises
is a hypothesis to check, not a defect to fix.

Scope every reviewer prompt: **one reviewer, no spawning, no clones, no remote commands.**
A reviewer with bypass permissions will otherwise clone the repo and start running suites
on a machine you did not assign it.

Chunk work small for xhigh — it dies under load, and a lane that dies mid-task costs more
than two lanes that finish.

## Fleet routing

**Probe load before dispatching.** The control plane saturating while the fleet sits idle
is the default failure, not an edge case. Check load, memory, disk, and running jobs on a
target before sending work — online is not the same as idle or qualified.

Assign **a specific box per lane in the brief**, not "use the fleet." Use warm
pre-provisioned worktrees; never clone fresh. Record every PID you launch and kill only
those — never by name or pattern, on any machine, ever.

Every wait must be harness-tracked (a background task whose exit re-invokes you, or a
bounded foreground loop). A detached watcher is killed silently and the lane strands.

## The 20-minute tick

Every 20 minutes until the program is done, check in. **This tick is for judgment, not
polling** — harness-tracked work already re-invokes you when it finishes. The question the
tick answers is *"is this approach still right?"*, not *"is it done yet?"*

Report exactly this, in text:

```
PROGRAM: <name>                                     <N>% · ETA <duration>

  <milestone>            <N>% · ETA <duration> · <one line>
  <milestone>            <N>% · ETA <duration> · <one line>
  ...

DECIDED WITHOUT ASKING
  <decision> — <why, one line>

NEEDS YOU
  <the questions list, per the protocol below>
```

**Percent must be countable.** It is *N of M concrete checkable items*, and the check-in
says what M is. A percentage derived from feeling is worse than no percentage, because it
never moves and nobody can tell.

**Report ETA movement, not just ETA.** `ETA 40m (was 20m, second slip)` is the useful
form. A milestone that has been twenty minutes away for two hours is the single most
important thing on the page, and a bare current estimate hides it perfectly.

## Questions protocol

When you genuinely need the user, put every open question in **one plain-text list**. No
multiple-choice UI, no one-at-a-time. For each:

1. **ELI11** — what the choice is, in language that assumes no context on this codebase.
2. **Your recommendation.**
3. **Your confidence, as a number.**

**Above ~80% confidence, just decide.** Do not ask. But log it under `DECIDED WITHOUT
ASKING` in the next check-in, with the reason — an autonomous decision nobody can see is
not autonomy, it is a surprise. The user gets a veto without paying for a round trip.

Batch questions to the tick. Interrupting between ticks is for a hard block only.

## Escalate the approach, not the task

Stop and rethink — do not grind — when any of these fire:

- A milestone reports the same percent on two consecutive ticks.
- An ETA slips twice.
- A lane has produced no commit in 40 minutes.
- A fix loop has run three rounds. Three is the limit; then descope, change approach, or
  ask.
- A check surfaces one defect per run. That is a broken instrument, not a stream of bugs —
  fix the instrument to report everything at once before running it a fourth time.

## Invariants

- Never `git stash` — the stash stack is global across worktrees and will apply another
  lane's work into yours. Use a temporary commit.
- Two independent implementations plus a diff beat either one's test suite. Worth it when
  a piece is subtle and central.
- When a reviewer's finding count barely moves between rounds, stop delegating the
  diagnosis and do it yourself.
- Verify by running the thing, never by grepping for it or type-checking it. A syntax
  check passes on code that cannot run.
