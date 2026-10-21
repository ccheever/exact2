# LLP 1124 (host extensions), round 1 blind review: GPT-6 Astra, high

Reviewed at PR #567's head `fdf91d204` (then numbered 1123), 2026-10-16. The number-collision finding is resolved by the renumbering to 1124.

1. **Should-fix — [llp/1123-host-extensions.rfc.md:1](llp/1123-host-extensions.rfc.md:1): LLP number collides with current main.**  
   `origin/main` (`c3c0bd8e0`) already contains `llp/1123-content-owners-and-local-safe-areas.rfc.md`. Merging produces two unrelated LLP 1123 documents, making numbered references ambiguous. Renumber this RFC and update its PR description.

2. **Should-fix — [llp/1123-host-extensions.rfc.md:157](llp/1123-host-extensions.rfc.md:157): Synchronous sources have no valid web route.**  
   D5 requires a source marked `sync` to use `call`, while D6 explicitly permits only `later` and `connect` on the web. An extension declaring a synchronous status source and providing both Apple and web implementations passes D9’s implementation-presence requirement, but its web source cannot execute as specified. LLP 1067.000 D9 handles this with a `later` fallback; this proposal removes the app wrapper that supplies it. Specify a runner fallback or explicitly reject such sources at web build time.

The supplied record contains no earlier review comments. The diff is documentation-only and leaves `llp/current/` unchanged. The external spike results are reported without artifacts available in this PR, so I could not independently verify those execution claims. No tests were run.

**Recommendation: MERGE AFTER FIXES** — resolve the LLP-number collision and define the web behavior for synchronous sources.

VERDICT: FINDINGS
