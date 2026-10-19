# LLP 1132 — consensus and writing check of de60ddbcec by Sol (gpt-6.1-sol, max), 2026-10-18

Critical: none. Major: none. No approval-blocking minor findings.

The four clarity edits restate existing rules:

- D2 already limits cancellation to measurements still pending.
- D1’s “input’s own action” rule and D6’s overdue-timer example already give timer and response-handler commits cause `program`.
- D2 already specifies shown-then-cancelled for marked visits beginning after a terminal failure.
- The original sheet-route definition already includes both `modal` and `fullscreen`.

I compared every sentence changed by de60ddbcec with its predecessor. I found no meaning change. The consolidated visit-beginning definition gathers existing rules from Visits, Start and Shown. UIKit’s transition-start exception survives in Start; response redirects retain commit timing through `program`. The frame rule retains the pending-frame choice, Linux’s no-frame fallback and the prohibition on using a later input’s frame. The push bullet preserves both the sheet-route and split-view exceptions. The remaining substitutions, journal wording and test split preserve their predecessors’ meaning.

These are minor, nonblocking writing notes; the surrounding explicit rules resolve each reading:

1. **Qualify cancellation consistently.** [D3, line 298](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:298): “Unmounting a section cancels its measurement.”

   This is broader than D1’s precise rule: unmounting ends every measurement but cancels only a pending one. It leaves the same opening that the panel-deselection edit just closed.

   Replace with: “Unmounting a section ends its measurement and cancels it if it is still pending.”

2. **Use the same causal test for section mounts.** [D1, line 147](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:147): “the start of what mounted the section: the input whose dispatch led to the mounting commit, or the commit itself when there was no such input”.

   “Dispatch led to” is looser than Cause’s “own action made the commit”. An overdue timer can run within that dispatch, as D6 explicitly warns.

   Replace with: “the timestamp of the input whose own action made the mounting commit, or the commit’s time if no input action made it”.

3. **Place the no-frame exception outside the two branches.** [D2, line 203](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:203): “A host that has no frame pending and schedules none for the change, because it draws nothing that changed, also uses that moment, as Linux can beneath a cover”.

   This retains the old exception, but its position under “If it did not” invites readers to restrict it to unchanged content. The covered-change test confirms that it also applies when content changed.

   Move it after both bullets and replace with: “Even when content changed, use the moment the conditions came to hold if the host has no frame pending and schedules none because it draws nothing that changed. Linux can do this beneath a cover.”

4. **Distinguish a visit beginning from its timing origin explicitly.** [D1, line 83](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:83): “A visit’s **start** is the moment its cause happened.” Elsewhere: “A dialog visit starts when a `dialog` opens”.

   The screen definition deliberately distinguishes beginning from an earlier measured start, while the dialog sentence uses “starts” for beginning. Readers must reconstruct that distinction.

   Replace the Start opening with: “A visit’s **start** is its timing origin: the moment its cause happened. It can precede the moment the visit begins. Exact measures shown and interactive times from this origin:”

   In the dialog definition, replace “starts” with “begins”.

5. **Keep “sheet route” distinct from “sheet”.** [D1, line 49](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:49): “That presentation contains the sheet and the routes that follow it, up to the next sheet”.

   The preceding definition now expressly makes a sheet one kind of sheet route, alongside a fullscreen route. This sentence still uses the narrower word for both kinds.

   Replace with: “That presentation contains the sheet route and the routes that follow it, up to the next sheet route.”

6. **Remove the overly broad description of late marks.** [D7, line 439](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:439): “A mark that becomes true that late is treated as one that never did.”

   Only Observe’s emission policy treats it this way; Exact retains the interactive event. The preceding two sentences already state both facts precisely.

   Delete this sentence.

READY