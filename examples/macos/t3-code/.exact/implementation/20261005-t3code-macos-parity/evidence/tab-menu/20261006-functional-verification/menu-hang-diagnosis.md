# Additional integrated runtime finding

Root drove real right-click on file tab, observed native menu, then AXPress Copy path. Menu disappeared but clipboard did not become the expected fixture path; 20-second settle refused outstanding native request. No unrelated clipboard content was inspected or copied into evidence.

`target/t3-verify/gui/copied-tab.json` reports localChanged ticket350 pending. Later `menu-pending-final.json` pending[] is **not** successful completion: `menu-logs.json` explicitly forgets request350 immediately before a different chatLocal press requests351. There is no fulfil350. The new mutation replaced the hung mutation. `menu-hang-log-extract.txt` preserves this exact sequence.

A read-only one-second `sample 65068 1 1 -file /tmp/t3-tab-pending.sample` while pending showed normal idle main AppKit loop, no modal NSMenu stack. This is consistent with the native menu having returned while the data request remained unresolved; it does not prove where the reply was lost.

Source trace: T3ContextMenu.perform owns menu target across blocking show/popUp, and then always calls reply with clicked value or null. showTabMenu awaits generation-checked native contextMenu; surfaceLocal then awaits recursive copy-path; copyTabPath awaits native copyText. No detached promise appears on this path. Native copyText writes pasteboard and returns copied flag; no evidence proves execution reached it here.

Verdict for this attempt: integrated menu interaction did not complete and verification fails this observation. Root cause remains unproven; do not attribute it definitively to X14 or clipboard API. A cancelled menu should also complete, so even AXPress behaving as dismissal does not explain a permanently pending request. The earlier policy/native-template tests remain passing within their stated scope.
