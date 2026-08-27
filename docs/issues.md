# Issues

An issue is a markdown file. Open issues live in `issues/YYYYMMDD-slug.md`. Closed ones
live in `issues/closed/` and carry a one-line resolution. That is the whole mechanism.

The official word is **issue** — the directory is `issues/`, the tool is `issue.mjs`, the
command is `issue new`. People will say "ticket" and everyone will know what they mean;
just don't name anything that.

## Why the filesystem

A tracker is a second place where truth lives, reachable only by a human with a login.
Filesystem issues are in the diff, greppable, editable by anyone working the repo —
agents included — and they travel with the branch that fixes them. The cost of filing one
is low enough that things actually get filed.

The tradeoff is real and worth stating: you lose assignment, cycles, boards, and
cross-repository views. **When an issue grows into work that needs those, graduate it** —
move it to a real tracker, record the pointer in the file, and close the filesystem copy.
Do not track the same live state in two places.

## The header

```
# <one-line title>

**Status:** Open
**Systems:** <comma-separated>
**Author:** <who filed it>
**Date:** <YYYY-MM-DD>
**Resolution:** <one line>        required only under issues/closed/
```

Optional: `Severity`, `Related`, `Marker`. Optional all-or-none triage block:
`Impact`, `Urgency`, `Ease`, `Confidence`, `Score reviewed`, `Score rationale`.

**`Status` is exactly `Open` or exactly `Closed`.** The value is the whole rest of the
line, so `Closed (fixed upstream)` is a *different status*, not a qualified one. Put the
qualifier in `Resolution`. This is the single most common way the format breaks, and the
checker moves your words to the right field rather than rejecting them.

## Commands

```sh
node scripts/issue.mjs new "<one-line title>" --systems "<a, b>"
node scripts/issue.mjs close issues/<file>.md --resolution "fixed by <sha>"
node scripts/issue.mjs check --fix
```

**Do not close an issue with a bare `git mv`.** The closed path requires `Status: Closed`
and a non-empty `Resolution:`, and a bare move is the most common way the format check
goes red. `--fix` repairs mechanical problems — it moves your own words between fields and
never invents a resolution.

## Wiring it up

`check` is the enforcement point. Register it inside one of your existing blocking checks
rather than adding a sixth — it is fast, deterministic, and reports every malformed issue
in one run rather than stopping at the first, which is the behavior every check in this
stack is required to have.

## What this is not

Not a workflow engine. There are two states. There is no triage queue, no assignee, no
sprint, no automation that moves issues between states on your behalf. Those are what a
tracker is for, and reaching for one is a signal that this issue has graduated.
