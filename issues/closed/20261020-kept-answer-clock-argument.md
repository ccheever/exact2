# A kept answer is refused at every launch when a resource takes the clock

**Status:** Closed
**Resolution:** lower-resource-launch-identity names an argument equal to epochAtZero or seed at launch (one plus or minus a value zero then), as the runner did; the identity rule is unchanged
**Systems:** runner (kept answers), Contract (resource), LLP 1027.005
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

`resource inbox = loadInbox(user, now, reloads)`, with `now` from `exactTime()`: every launch
refused the kept answer, because the clock argument differs from the one it was kept under, so the
app opened empty until the network answered. The log said exactly why (`with now`), and the lane
fixed it in five minutes by leaving the clock out of the arguments.

## Question

Whether this is a bug or the intended rule is for the owner of kept answers (LLP 1027.005) to say.
A clock is the commonest argument a list source takes (relative times, "due today"), so the default
makes such a source cold at every launch, and an author learns it only from the log. Options: a
clock argument does not take part in the kept answer's identity; or `contract build` warns when a
`resource` whose answer is kept passes a clock.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-19),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet re-run on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` on Charlie's Mac.

## Resolution (2026-10-20)

The identity rule is intended (LLP 1027.005: a value of this launch's own goes after `with`), and the
runner already named this case at launch: it compares each identifying argument with the epoch,
and the Linear Port's `now = time.epochAtZero + tick * 60000` equals it while `tick` is 0. The
compiler's `lower-resource-launch-identity` followed only `epochAtZero` and `seed` as written, so
it missed it. It now also names one plus or minus a value that is zero at launch (a literal 0, a
state starting at 0, a product with one), the runner's test at compile time. A time that differs
from the epoch at launch (`time.epochAtZero + 60000`, `+ performanceNow()`) is still not named:
it may be what the fetch is about. Tests: `contract/cli/tests/it/launch_identity.rs`.
