# LLP 1132 — consensus round 2 by Sol (gpt-6.1-sol, max), of 25d6a0b0f4's successor at the Luna clarifications, 2026-10-17

No critical findings. I found no unintended change to the operative rules in the writing pass. The failure timestamp, hidden-before-readiness rule and Observe’s entry-plus-URL history clarification check out.

1. **Major — D1, Visits: the hidden-route clarification contradicts itself.**

   [RFC:45](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:45) defines a “sheet route” as either `modal` or `fullscreen`. [RFC:56](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:56) then says both “A fullscreen route hides the route beneath it” and “A route just beneath the top sheet is not hidden.”

   This changes the measured time. Suppose previously shown A is beneath fullscreen B. At 300 ms, A’s mark becomes true and its text changes. Treating A as hidden records 300 ms; treating it as visible waits for a frame showing A’s change, potentially until B closes. The web explicitly distinguishes `modal` from fullscreen when preserving the predecessor’s visibility: [navigation.js:308](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:308), [navigation.js:316](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:316).

   **Fix:** qualify the visible-predecessor statement as applying beneath a **nonfullscreen modal sheet when the host continues presenting that route**. Use the host’s presentation state to distinguish hidden from merely dimmed or inert. This needs a wording correction, not additional occlusion machinery.

2. **Major — D1, Visits: retaining the router entry does not guarantee retaining the route element or its mark.**

   [RFC:64](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:64) says a `replace` keeps “the entry, and so the route element and its mark.” The cited router code preserves the entry ID while replacing its **name**, URL and parameters: [router.rs:182](/Users/admin/Developer/observe-lab/exact2/route/src/router.rs:182).

   The documented authoring pattern conditionally renders route elements by `e.name`: [navigation-stack.md:56](/Users/admin/Developer/observe-lab/exact2/docs/recipes/navigation-stack.md:56). Replacing `/note/1` with `/add` at depth two therefore changes branches. The runner destroys the old branch and realizes the new one: [region.rs:337](/Users/admin/Developer/observe-lab/exact2/runner/src/instance/region.rs:337). The web likewise ends and rebuilds conditional scopes: [rt.js:1189](/Users/admin/Developer/observe-lab/exact2/host/web-js/rt.js:1189).

   The destination can consequently have a different mark—or none. Carrying the old mark forward would measure the wrong declaration.

   **Fix:** say that `replace` retains the **entry ID** and starts a new visit when the URL changes. **If the route element remains mounted**, it retains its mark; otherwise ordinary mount and unmount rules apply. The retained-element scenario should state that premise explicitly. No new lifecycle mechanism is needed.

3. **Minor, nonblocking — Decisions: remove naming-process history.**

   [RFC:513](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:513) recounts who agreed on the name and at whose request. That adds process history where CLAUDE.md asks for the reader’s needed information.

   **Fix:** retain the explanation that “mark” denotes measurement and the discussion link; remove the agreement narrative. This merits a sentence edit, not another rule.

NOT READY