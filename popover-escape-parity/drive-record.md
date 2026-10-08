# popover-escape-parity: agent drive record (2026-10-08, 13:00-13:35 KST)

Host: this Mac, agent-mode macOS app through `scripts/agent.mjs` `open()` at 1280x840 (840x620 for the narrow
rows), `--epoch` 1791432540000 (the fixture's clock), driven op by op (`target/pep/drive.mjs`, uncommitted,
from usage-pooled-view's). The pairing link was typed from a 0600 file and redacted from every transcript.

| Item | Value |
| --- | --- |
| Lane T3 servers | staged release `t3` 0.0.46-nightly.20261005.2667: "Studio" 127.0.0.1:16580 (pid 48702), "Build box" 16590 (pid 48704); isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME each; telemetry off |
| Fixture proxies | usage-pooled-view's `fixture.mjs` (uncommitted): 16581 -> 16580 (pid 48703), 16591 -> 16590 (pid 48705). Build box pairs; it shows two Codex accounts (Codex `shared@`, Work `work@`, 1 banked credit), Cursor's Keychain offer and the ChatGPT link |
| AFTER build | session 1: this branch before the ground and the own-press fix (app pid 3041); session 2 (the one retry): this branch's final contract (app pid 75713), `target/clients/586a72264334ae032519b9f4` |
| BEFORE build | `t3-code-evidence-base` at `07dcef1ab` (its bundle `target/clients/0d02a3550eba6edce4b12b59`, app pid 98359), the `.build-lock` held for the drive |

Each row is read back from the `tree` reply: a segment's `aria-expanded` (`accessibilityExpanded`), the
focused node, whether `theme-editor`, `close-settings` (Settings) and `usage-page` exist; the journal
(`logs`) names the actions each press ran.

## Session 1 (AFTER, found two defects)

- A press on the page's empty ground under the cards (`tap usage-scroll clicks 1 at 600 700`, below the
  604-pt content in a 788-pt port) left the popover pinned and journalled no `pointerdown`: on macOS a press
  on a scroll view's empty area reaches no node. Fixed: a full-height ground (`usage-ground`) under the
  content. Recorded in `EXACT2-GAPS.md` as a host finding.
- A second press on a pinned segment kept it open when the press counted twice, once as the window's outside
  press. The segment lay under a toast ("Update Available", then "Nightly needs the beta mobile app"): the
  toast takes that click, so a real hand presses the toast there. Re-run without toasts (session 2): passes.
  Fixed on the way: a press on the popover's own segment now also clears the hover states, so a real pointer
  resting on the segment no longer holds it open (base: image 07).
- Theme editor (unchanged since): Escape in Theme name left Settings and kept the editor; a second Escape kept
  it; Settings reopened over the editor, Escape left Settings and kept the editor; Escape in the Accent
  colour popover closed it and focused `theme-editor-swatch-accent`.

## Session 2 (AFTER, the retry; all rows)

| Row | Ops | Read back |
| --- | --- | --- |
| Pin | `tap usage-seg-0-0-0` | 0-0-0 expanded, focused (`press view 878 (pin)`) |
| Ground under the cards | `tap usage-scroll clicks 1 at 600 700` | closed (`pointerdown/up view 785 (pressDown/pressUp)`); layout `usage-ground` 1024x788 = the port |
| Card body | pin, `tap usage-pooled clicks 1 at 168 52` | closed |
| Another segment | pin 0-0-0, `tap usage-seg-0-0-1` | 0-0-1 expanded, 0-0-0 not |
| Own segment | pin 0-0-0, `tap usage-seg-0-0-0` | closed, the segment keeps the focus |
| Own segment, 2nd card | pin 0-1-1, `tap usage-seg-0-1-1` | closed (toasts dismissed first) |
| Inside the card | pin, `tap usage-seg-pop-0-0-0 clicks 1 at 100 150` | still expanded |
| Right-click outside | `tap usage-pooled contextmenu at 168 52` | still expanded (a secondary press is no click) |
| Escape | `type usage-seg-0-0-0 key Escape` | closed, focus on the segment, Usage still up (`press view 787 (escape)`, `focus("usage-seg-0-0-0")`) |
| Sidebar (outside the page) | pin, `tap sidebar clicks 1 at 120 500` | closed (`pointerdown/up view 1 (outsidePressDown/Up)`) |
| Toast (outside the page) | pin, `tap toast-description-2` | closed |
| Narrow 840x620, legend | `tap usage-legend-0-0-0` | 0-0-0 expanded |
| Narrow, card text | `tap usage-pooled clicks 1 at 58 191` | closed |
| Theme editor over Usage | pin, `tap theme-editor clicks 1 at 60 110` | closed (the window's count) |
| Editor's swatch | pin, `tap theme-editor-swatch-accent` | usage popover closed, colour popover open, focus `theme-color-accent-saturation` (`pointerdown/up view 3574 (triggerDown/triggerUp)`) |
| Colour popover Escape | `type theme-color-accent-saturation key Escape` | focus `theme-editor-swatch-accent`; editor and Settings stay |
| Escape 1 in the editor | `type theme-editor-name key Escape` | Settings gone, editor stays |
| Escape 2 in the editor | same | editor stays |
| Settings reopened over the editor | `tap connection-settings`, `type theme-editor-name key Escape` | Settings gone, editor stays |

Not passing, pre-existing (#263's layout, not this task's): narrow, a second press on the same legend row
does not close the popover. The popover hangs off the segment and its 6-pt hover gap and top edge cover the
legend row's upper half (card at y 187, legend row 169-197), so the tap at the row's centre (y 183) lands in
the popover's gap and the legend's `press` never runs (no `pin` in the journal). The reference anchors the
popup to the trigger that opened it.

## BEFORE (base 07dcef1ab, same lanes, epoch, size)

| Row | Read back |
| --- | --- |
| Ground under the cards | still expanded |
| Sidebar | still expanded |
| Own segment (agent tap, no hover) | closed |
| Own segment with the agent pointer hovering (`tap usage-seg-0-1-0 hover`, then two presses) | still expanded after the second press (image 07) |
| Colour popover Escape | focus on the swatch; editor and Settings stay (same as AFTER) |
| Escape 1 in the editor | Settings gone, editor stays |
| Escape 2 in the editor | editor gone |
| Settings reopened over the editor, Escape | editor gone, Settings stays |

## After session 2: independent-review fixes (not driven; session budget)

`pressUp` also checks the release point (a press ending inside the card does not dismiss) and clears the hover
states with the pin; the segment's `pointermove` (it reopened a just-closed popover on any move) is removed, so
hover opens it again only on a new enter, as Base UI's `blockMouseMove`; the theme editor's text fields count
their `focus` as an outside press (a macOS text field reaches no `pointerdown`); Settings' Back and the Usage
page's back blur the focus first, as `useEscapeToGoBack` does. Unit-tested; the real-input steps in the task
record cover them.
