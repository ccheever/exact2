# LLP 1132 — terms check of 5ec9e50061 by Sol (gpt-6.1-sol, max), 2026-10-18

Critical: none. Major: none. No approval-blocking minor findings.

Every changed sentence preserves its predecessor’s operative meaning in context. All six cons13 notes are resolved: cancellation is limited to pending measurements; section mounts use the input’s own action; the no-frame exception follows both branches; Start distinguishes timing origin from beginning; presentations use “sheet route”; and the misleading late-mark sentence is deleted.

“Sheet route”, “fullscreen route”, “cover” and visit “begin” are consistent. The remaining exceptions and writing notes are minor and nonblocking:

1. **Some lifecycle sentences still use “start” for coming into existence.** D1’s explicit definitions resolve the intended meaning, but the cleanup remains incomplete.

   | Location | Quoted wording | Concrete fix |
   |---|---|---|
   | [D1:67](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:67) | “A `replace`'s new URL starts a new visit.” | “…creates a new visit.” |
   | D1:79 | “the input whose dispatch started it” | “the input that caused it”; retain the following sentence’s precise own-action test. |
   | D1:95 | “new visits start with the cause `foreground`” | “new visits begin with the cause `foreground`”. |
   | D1:124; D2:222,224; Tests:505 | “starts after them”; “starts after a failure terminal for it”; “starts after the failure”; “starts after poisoning” | Replace each “starts” with “begins”. These conditions concern a visit’s creation, which can follow its timing origin. |
   | D7:444 | “A visit that starts without a navigation” | “A visit that begins without a navigation”. |
   | Tests:477 | “ends the visit and starts a new one” | “…ends the visit and begins a new one”. |

2. **Section measurements have the same vocabulary overlap.** [D1:140](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:140) says “Selecting the panel again starts new measurements”; line 152 says “the next one starts”; line 155 says “A later mount starts a new measurement.”

   These describe creating a measurement, while its measured start can precede selection or mounting. Use “creates new measurements”, “the next one begins” and “creates a new measurement”, respectively. Keep “A measurement starts at the latest of these moments” for the timing origin.

3. **“Sheet” also means a dialog’s native presentation.** [D1:101](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:101) says “begins to present the dialog's sheet or alert”. This sheet is a presentation of a `dialog`, rather than the `navigationPresentation="modal"` route defined at line 45. The following “For a sheet” also leaves the reader to distinguish the host presentation from the route kind.

   Replace the iOS phrase with “begins to present the dialog”. On macOS, write: “For a route or dialog presented with `beginSheet`, it is the first frame after the host begins presenting it.” This preserves the rule for both kinds without overloading bare “sheet”.

4. **The launch analogy names the wrong timing origin.** [Decisions:543](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:543): “A navigation may still start at an input, as a launch starts at a tap on the app's icon (D2).”

   D1 explicitly uses process start for a native launch and page activation on the web. The icon-tap analogy therefore makes a careful reader stop and reconcile two origins. Replace the sentence with: “A navigation can still be timed from its initiating input (D1).” The precise Start rules already settle the behaviour.

READY