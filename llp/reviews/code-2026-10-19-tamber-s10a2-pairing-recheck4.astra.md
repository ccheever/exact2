# Quick re-check 4, LLP 1121 stage 10 A2, Astra

Reviewed: a4de2922fd (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

SHOULD-FIX — [scripts/tamber-scrub.mjs:140](/tmp/rv-rl-a2g/scripts/tamber-scrub.mjs:140): Post-await expiry checks end the scrub but return **409**, not **401**. Reproduced by expiring the client while planning was pending. Preserve the authentication failure status after each await.
