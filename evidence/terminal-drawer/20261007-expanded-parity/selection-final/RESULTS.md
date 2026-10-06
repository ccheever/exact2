# Native terminal selection and context

Actual normal macOS apps built from the recorded sources, physical CG mouse/key events, pinned server16331 and harmless native-fixture/default provider. No real provider request.

A reverse drag selected exactly PARITY_SELECT_ONE, PARITY_SELECT_TWO and PARITY_SELECT_THREE. The actual AppKit selection menu inserted a Terminal 2 lines 1-3 composer chip and returned focus. Copy produced exactly55 UTF-8 bytes, matching clipboard-settled.txt. The menu callback is asynchronous; the first immediate clipboard read preceded it, so the settled read is the evidence. The prior clipboard was restored.

Draft hover displayed the three captured lines. A normal quit/relaunch of the unchanged signed fixture restored both the chip and exact hover text without re-pairing. Sending through the selected fake provider produced one prompt containing one numbered terminal context; sent-user-request.txt contains only that request, excluding unrelated injected instructions. Clicking the sent chip opened the actual Lines1-3 preview with the same text.

The runtime check exposed two integration defects after passive selection-event forwarding was repaired: forbidden ambient new Date construction, then insertion at the end whenever the composer lost focus. The clock now comes from the existing snapshot clock; native insertion retains the selected range after blur. The terminal caller also stopped adding a second separator already owned by native insertion. Before-fix records remain preserved.

The final normal app uses the final-bundle.json executable. Physical paste prepared LEFT RIGHT; Cmd-Left then five Right keys placed the caret at5. Native accessibility confirmed5 before selection and5 after blur. The actual terminal menu inserted LEFT [terminal reference] RIGHT with exactly one separator. Focus returned to the composer. One Backspace removed the separator; the next removed the entire chip, restoring LEFT RIGHT and caret5. final-caret-proof.json and07/08 screenshots record these assertions.

These checks cover the recorded three-line selection, copy, insertion, persistence, preview, send and deletion paths. Multiclick/scroll/clamp, every UTF-16 caret permutation, and the64k GUI matrix remain outside this runtime sample. Component/native tests are recorded separately.
