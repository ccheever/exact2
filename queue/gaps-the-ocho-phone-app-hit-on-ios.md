**Gaps the Ocho phone app hit on iOS** (2026-10-03, `apps/ocho-mobile` on branch `ocho/client`; each worked around in the app):
- Data-source replies are parsed and committed on the main thread (`Session.swift`'s wake → `pump`): an 800 KB JSON reply every few seconds dropped frames while scrolling.
- A re-ask with new arguments drops the request in flight and its reply is never parsed (LLP 1016 D5), but nothing tells a source that keeps its own in-flight flag; an equal request is kept (LLP 1054.000.000 D3), so a source's retry of a hung request waits on the same one.
- `transition` cannot animate `filter` (only translate/scale/rotate/opacity); a `filter` entry in the list silently disables the whole transition.
- A native module view cannot report its intended size; a growing composer reports height through `message` and the app sizes its box.
- `backgroundMaterial="glass"` is a glass effect behind an Exact box, not UIKit's glass controls (`UIButton.Configuration.glass()`, glass containers that merge); the app uses native module views for buttons and its composer.
- A back swipe painted only the part of the incoming route visible as it began; fixed by painting each frame of the transition, but a device report says half the screen still fills in late.
