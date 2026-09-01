# Native hosts ignore disabled controls

**Status:** Closed
**Resolution:** Apple and Linux presenters now block focus, keyboard, pointer, change, press, and agent activation for disabled controls.
**Systems:** Apple host, Linux host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1007, LLP 1008, LLP 1015

The schema and Contract lower `disabled`, and the web host sets the HTML
property. Apple presenters never map it to `isEnabled` or an equivalent input
guard; macOS only omits disabled nodes from tab order while pointer dispatch
remains live. Linux press and text-input paths likewise never consult the prop.

Disabled buttons can therefore mutate state and disabled inputs can accept
text on macOS, iOS, and Linux, directly diverging from CSS/HTML behavior.

Apply disabled state in each presenter and gate every synthetic/agent input at
the same boundary as real input. Drive a shared fixture through tap, key, type,
focus, and accessibility traversal on all hosts.
