# exact deploy can call an unusable stream head current

**Status:** Closed
**Resolution:** fixed by shared client-parity head admission in classification and the locked publish recheck, with authenticated seq+1 repair
**Systems:** Delivery, exact deploy, Update store
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11; LLP 1030 D3; scripts/deploy.mjs

The deploy reader does not validate a live head by the rules the production
client uses. It checks the path compatibility id, app id, and non-negative
integer seq (`scripts/deploy.mjs:317-323`). `headSignature()` returns an error
as a table note; it does not make the row unusable. The reader does not reject
a different channel, unsupported `exact` major, unsafe/incomplete cards, or
other envelope-shape errors.

`changesAgainst()` then compares only the plan and asset digests. When those
match, the row is `current` (`scripts/deploy.mjs:325-333`), and `--yes` skips
it (`scripts/deploy.mjs:509`). Thus an unsigned, wrongly signed, wrong-channel,
or otherwise client-refused head can be reported current and can never be
repaired by rerunning the deploy with the same bundle. The locked recheck in
`publishStream()` repeats the same digest-only shortcut.

Done when the classifier and locked publisher validate a head against a
shared, fixture-parity implementation of every client admission rule. An
unusable head is either repaired with a newly signed, higher-seq head or is a
named refusal; it is never `current`. Cover invalid/missing signatures,
wrong channel, unsupported major, malformed cards, and a valid current head.
