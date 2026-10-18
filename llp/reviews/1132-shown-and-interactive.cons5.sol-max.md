# LLP 1132 — consensus check of the merged writing pass (a7b66ef3f7) by Sol (gpt-6.1-sol, max), 2026-10-18

1. **Major — D7’s corrected introduction also assigns framework diagnostics to Observe.**

   [D7](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:408) says: “Observe’s names and policies move from the framework to `modules/observe`… These move:”. That list includes “**Switches and output**”, where `EXACT_OBSERVE_LOG` becomes `EXACT_TIMING_LOG` and the agent’s `observe` section becomes `timing`.

   **Evidence:** These diagnostics belong to the hosts: [Agent.swift:293](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/Agent.swift:293) supplies the agent section, and [marks.js:302](/Users/admin/Developer/observe-lab/exact2/host/web-js/marks.js:302) supplies the web accessor. D5 also requires the timing log to print settled, which the RFC deliberately excludes from the journal consumed by Observe. “These move” therefore leaves two conflicting instructions: relocate those diagnostics into Observe, or retain and rename them in the framework.

   **Fix:** End the relocation list before “Switches and output”. Introduce that group separately: “The framework’s diagnostics are renamed as follows:”.

2. **Minor — The appearance-gate correction does not fully state its platform scope.**

   [D4](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:316) says: “On iOS, the screen visit in place at launch waits for UIKit to show it (`routeAppeared`)”. The Apple implementation row still says, without qualification, “the launch route gated on `routeAppeared`”.

   **Evidence:** The existing navigation gate is compiled under `#if os(iOS) || os(tvOS)` in [NavigationMarks.swift:272](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/NavigationMarks.swift:272). Its controller implementation likewise covers both platforms. The distinction is UIKit versus AppKit; it is not iOS alone.

   **Fix:** Say “On iOS and tvOS” in D4, and qualify both `routeAppeared` clauses in the Apple implementation row with the same scope.

No critical findings. I found no additional semantic change from the writing pass. Citations, links, headings and the meaning of all 34 tests are preserved. The Apple router-op description, router commit timestamp, and retention of raw marks for an unshown launch agree with the code.

NOT READY