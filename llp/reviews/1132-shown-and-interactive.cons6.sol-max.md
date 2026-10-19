# LLP 1132 — consensus check of af1ebd4048 by Sol (gpt-6.1-sol, max), 2026-10-18

1. **Major — D7 still contradicts the diagnostics it retains.**

   **Quoted text:** “The framework keeps only the facts those policies read.” ([RFC:408](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:408))

   **Evidence:** V1 said “keeps the facts those policies read,” without **only**. The added restriction excludes `settled`: D5 requires the framework’s timing log to print it, while the summary explicitly excludes it from the journal consumed by Observe. The new diagnostics paragraph correctly retains those diagnostics in the framework, creating conflicting instructions within D7. Their framework ownership is confirmed by [Agent.swift:293](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Agent.swift:293) and [marks.js:302](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:302).

   **Concrete fix:** Delete **only**, restoring “The framework keeps the facts those policies read.” Keep the separate paragraph retaining and renaming framework diagnostics.

The two previously reported findings are otherwise resolved: diagnostics sit outside the relocation list, and the appearance gate correctly names iOS and tvOS. Apple’s router-op description, the router event’s commit timestamp, and retention of raw marks for an unshown launch agree with the code and the proposed timing rules.

I found no other semantic drift or writing issue that would block approval. Headings, links, file citations, and all 34 tests retain their content; the single test wording change preserves its meaning.

NOT READY