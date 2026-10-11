# Apply autofocus when a view mounts after initial JS boot

**Status:** Closed
**Resolution:** Fixed by ecd3cfdde and follow-up focus fixes; 18 focus tests pass, including fields mounted after boot.
**Systems:** web JS runtime, focus parity
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** QUEUE.md autofocus; LLP 1035.000 D9; host/web-js/rt.js:1416

The JS target applies autofocus only once in `mount`, after its initial boot. Views inserted later by conditional regions do not receive the shared mount-time focus behavior.

Built a current JS page with a false `shown` state, a “Show” button that sets it true, and `when shown` containing `input autofocus=true testId="later"`. In Chrome, clicking Show mounted the input successfully, but `document.activeElement.dataset.testid` remained “show.” The generated input carried the autofocus attribute. This is the existing QUEUE observation, now reproduced against the current compiler and browser.

LLP 1035.000 D9 says autofocus is honored at mount, with one-per-document semantics and special handling for a carried-state restart. The wasm host has a mount/commit focus controller. `rt.js` only queries `root.querySelector("[autofocus]")` at lines 1416–1418.

Track autofocus candidates as views mount and apply the shared rule after their commit, with the same retainFocus and restart restrictions as the other host. Avoid repeatedly stealing focus during updates or adoption.

Acceptance: first autofocus mounted through a conditional region is honored; later candidates, retainFocus contacts, carried-state reload and document adoption match the wasm/native decided behavior. Add the fixture to step-by-step conformance rather than only a boot snapshot.
