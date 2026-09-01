# Web `clock settle` waits on in-flight fetches with no deadline

**Status:** Closed
**Resolution:** Agent clock settlement now carries a deadline and refuses non-settling motion.
**Systems:** Web host, Agent API
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1012 §2 (native hosts bound settle at twenty seconds and return `settled: false`)

```
async function clock(request) {
  const settle = !!request.settle;
  for (let rounds = 0; ; rounds++) {
    if (settle) while (inflight.size) await Promise.race([...inflight]);
```

The web `while` has no timeout. A hung `fetch` hangs the agent. The 16-round cap is only reached after every fetch completes. Apple/Linux pump the executor up to 20 s and return `settled: false`.

Fix: `Promise.race` against a 20 s timer; then `settled: false` as on Apple.
