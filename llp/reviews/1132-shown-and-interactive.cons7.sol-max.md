# LLP 1132 — consensus check of 999f709b9e by Sol (gpt-6.1-sol, max), 2026-10-18

No critical findings.

1. **Major — changes to mounted input requirements lack an evaluation trigger.**

   **Quoted text:** “Each of the following triggers an evaluation” ([D2:191](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:191)); “its unmount uncounts it” ([D6:383](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:383)).

   **Evidence:** Condition 2 now depends on the currently mounted subtree. D2’s triggers omit changes to that subtree’s input requirements. D6 describes updating counters, but does not require reevaluating affected pending marks.

   Consider a shown screen marked literal `true`, with activation and entrance complete, waiting only for its custom pressable’s delayed input module. The pressable unmounts without changing the mark. The screen should become interactive at the frame showing the removal. No listed trigger evaluates it then; an implementation following that schedule can wait for the now unnecessary module indefinitely.

   The converse also matters: mounting a descendant that needs an already failed module must apply `input_failed` to an existing pending measurement. The explicit post-failure rules currently cover measurements that **start** after failure.

   **Concrete fix:** Add changes to a mark’s mounted input requirements as an evaluation trigger. After each accepted commit, update the counts and reevaluate affected pending enclosing marks for readiness and retained failures. Invalidate a pending frame candidate if readiness is lost. Apply D4’s services rule when the affected measurement is the launch root. This remains O(depth), without a tree read. Add regressions for removing the last dependant during a delayed load and mounting a dependant after a retained failure.

2. **Minor — D3 states section independence too broadly.**

   **Quoted text:** “A visit’s interactive time does not wait for its sections.” ([D3:286](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:286))

   **Evidence:** D2 now expressly includes input requirements inside nested sections, and the new web test requires a section’s custom pressable to delay its screen. D3’s sentence can therefore suggest an exemption that the settled decision rejects.

   **Concrete fix:** Write: “A visit does not wait for its sections’ readiness expressions or interactive times. Its input requirements still include all mounted descendants, including those inside sections (D2).”

84c5a924fc resolves the “only” finding, and both af1ebd4048 fixes stand: diagnostics remain in the framework, and the appearance rules name iOS and tvOS. The cited activation callback and input-loader sites support their claims; the foreground-inactive statement is correct. The Expo quote is exact, and the documented difference between Expo Router, React Navigation and the documentation matches the supplied sources.

The app-entrance and foreground-active decisions otherwise agree with the launch, cancellation, services, sheets and no-pause rules. I found no further blocking semantic or writing regression in the full RFC diff from `llp-1126-v1`.

NOT READY