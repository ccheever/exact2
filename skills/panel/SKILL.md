---
name: panel
description: Take a hard directional decision, or an RFC/LLP that needs refining, to the panel — four frontier models from different families, mutually blind, one pass (two at most), asked only about fundamental and directional issues. Use instead of llp-super-refine in almost every case. Use when the user says "take it to the panel," "ask the panel," or when a fork in direction is too consequential to decide alone.
---

# panel

Four models from different families, asked the same question, blind to each other. You go
to the panel to get the **direction** right — not to finish the design.

## Who sits on it

| Seat | Command |
|---|---|
| Astra, max | `codex exec -m gpt-6-astra -c model_reasoning_effort="max" "<prompt>"` |
| Fable, xhigh | `claude -p --model claude-fable-5-1 --effort xhigh "<prompt>"` |
| Grok 4.7, xhigh | `grok -p "<prompt>" -m grok-4.7 --effort xhigh` |
| Muse Wonder Full, xhigh | `muse exec --model muse-wonder-full --effort xhigh --trust-workspace "<prompt>"` |

Muse sits **if available**. If it fails to start, the panel is three and the report says so —
do not stall the decision waiting on a seat. The other three are required; if one is down,
say which and ask whether to proceed short.

Run all seats in parallel, as harness-tracked background tasks, on the same revision of the
same text. No seat sees another's answer.

## When to go

- **A hard decision about direction.** Two architectures, a build-vs-buy, whether to change
  course mid-program. The kind of fork where being wrong wastes the work built on top of it.
- **Refining an RFC or LLP.** One pass before implementation starts.

Not for code review — that is orchestrate's two-family review, on a diff. Not for anything
you would decide above ~80% confidence on your own.

**The panel replaces `llp-super-refine` in almost every case.** Super-refine iterates until
two families agree a document is ready, which spends rounds polishing details that are
cheaper to settle in code. Reserve it for planning a genuinely complex system where a
foundational error would be ruinous and impossible to see until late — and say why when
you reach for it.

## What to ask

**Foundational, directional, structural issues only.** Is this the right problem? The right
approach? Is there a load-bearing assumption that is wrong, a simpler shape that does the
same job, a constraint that kills it? Those are what the panel is for.

Nitpicks are explicitly out of scope. Naming, edge cases, API details, missing error paths —
they get worked out during implementation, where they are concrete and easier to judge.
Resolving them in prose first is effort spent twice.

Every seat gets the same prompt, scoped like this:

```
You are one reviewer on a panel. Do not spawn agents, clone anything, run remote
commands, or edit files. Read only.

<the document, or the decision with its context and the options>

Report only fundamental issues: wrong problem, wrong approach, a load-bearing
assumption that does not hold, a materially simpler design, a constraint that
breaks it. At most five, most serious first. For each: the issue, why it is
fundamental, and what you would do instead.

Do not report nitpicks, naming, edge cases, or details that implementation will
surface. If you have no fundamental concerns, say so — that is a useful answer.
For a decision: state which option you would choose and your confidence as a
number.
```

## Passes

**One pass by default.** Run a second only when the first changed the direction — the
revised direction deserves a look of its own. Never a third: if two passes have not
converged, the disagreement is the finding, and it goes to the user.

## Reading the results

- **Agreement across families is signal.** Three seats raising the same fundamental issue
  is close to a fact.
- **A concern only one seat raises is a hypothesis.** Check it yourself before acting on it.
- **A split on direction is not a vote to count.** Bring it to the user with each side's
  strongest argument.
- Anything a seat raises that is below the bar goes on a list for implementation, not into
  another round.

For an LLP, record each review actually received the way `llp-review` does — one
provenance-tracked artifact per seat, never fabricated, never summarized into one.

## Report

```
PANEL: <question or document>        <seats that sat> · pass <N>

DIRECTION     <what the panel converged on, or "split">
AGREED        <fundamental issues raised by two or more seats>
SINGLE-SEAT   <issues one seat raised — checked: held up / did not>
SPLIT         <where seats disagreed on direction, each side's best argument>
DEFERRED      <below-the-bar items, left for implementation>
NEXT          <revise and implement / second pass / needs you>
```

Then move. The point of the panel is to start implementation pointed the right way, not
to start it later.
