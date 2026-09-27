# Measure a 1,000-row list whose rows declare `transition: all`

**Status:** Open
**Systems:** Heavy-list harness, `exact-motion`, Apple/web hosts
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062 D2/D3; the memory work in QUEUE

LLP 1062 D2 says only a node that names paint owns it, so 1,000 coloured rows cost nothing. But `all` names every property, so rows that declare `transition: all`, as web authors routinely do, each become paint owners, with engine slots per property (about 104 B each) and whole-style re-sends on Apple.

**Do:** measure memory and frame time for 1,000 rows with `transition: all` on the heavy-list harness, on iOS and the web. If the cost is real, adopt ownership lazily, at the first change of a paint target, rather than at declaration.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
