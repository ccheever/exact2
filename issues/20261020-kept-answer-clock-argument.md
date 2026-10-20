# A kept answer is refused at every launch when a resource takes the clock

**Status:** Open
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
