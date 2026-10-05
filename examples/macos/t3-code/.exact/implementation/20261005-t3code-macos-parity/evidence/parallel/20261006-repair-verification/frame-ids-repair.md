# Frame lookup correction and final verification boundary

The two-read-slot build's actual loaded-output replay revealed that Down at the bottom jumped to zero. The measured native viewport and content were correct (320 and 654.875 points). Independent inspection of `runner/src/geometry.rs` showed that `frame()` finds authored `id` values; the two nodes had only `testId`. Both expressions therefore returned unavailable geometry with zero height. The final correction adds matching `id` attributes to the output and content nodes. The measured clamp formula and read flow are unchanged.

`checks-frame-ids/report.json` records the complete static suite passing against unchanged final sources. The named `T3 Code (Exact).app` rebuilt successfully; `frame-ids-bundle-check.json` records the executable and receipt with no changed inputs before launch. Final boundary replay is recorded separately in the timeline evidence.

The previous apparent request serialization is not a proved production defect. Independent review found that mutation `then` callbacks run on the next clock advance, while the agent driver waits for outstanding replies before advancing and disables automatic time. A second AX disclosure during that wait can publish its loading state, but its callback cannot dispatch until the first wait ends. The frozen-clock test cannot establish real-time request overlap. Keep that acceptance case blocked until a supported real-time run is available; do not treat unit tests or distinct request ownership as equivalent proof.

No further speculative repair is attempted. Earlier failed evidence is retained, with its capture limitations corrected in the independent review and latest task records.
