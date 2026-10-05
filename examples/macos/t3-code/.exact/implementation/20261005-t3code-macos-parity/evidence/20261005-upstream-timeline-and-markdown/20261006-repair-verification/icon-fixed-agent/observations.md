# Mounted acceptance after the native icon closure fix

Final rebuilt app PID 1110, isolated agent mode, reference server 16843 through controlled proxy 16844. App source was not edited during capture. Operations use supported native agent input and semantic AXPress. There is no physical OS wheel claim.

## Passing subset

- The native-app asset request completes without the previous Hermes `set` exception; native icon source mutation settles.
- Actual sequential Tab walk reaches the result through hidden image hook → timestamp → hidden tooltip hook → output. Space collapses and Enter opens the command; focus stays on disclosure. Hidden hooks remain additional stops.
- Prompt disclosure is no longer blocked: semantic AXPress opens B and shows Loading output while A remains pending.
- Actual result scrolling: wheel resets native offset159.75→0; ArrowDown moves0→40; wheel to bottom reaches334.88. `visible-*.json` reads the text child so its scroll-ancestor entry includes actual output scroll1623; authored scrollTop alone is not treated as proof.

## Remaining observed problems

**Opening B during A does not send B's detail read within the active read mutation.** The verification proxy delays `fixture-broken` by15seconds and `fixture-failed-icon` by2seconds. A request is at17:40:52.798Z. A preprogrammed helper initiates numeric clock and concurrently AXPresses B at17:40:53.938Z, without model/tool delays. Its returned AX tree has both expanded/loading. At17:40:57.438Z both still show Loading. Trace contains only A's detail RPC during the read, and clock-completion tree has A output but B still loading. The previous sequential-clock attempt is excluded because numeric clock awaits asynchronous work. This corrected attempt does prove the overlapping user action; it does not prove two concurrent requests or B finishing first.

**ArrowUp at a clamped bottom does not move immediately after extra ArrowDown presses.** Native offset is334.88 at bottom, remains334.88 after three Down presses, and remains334.88 after Up. Expected immediate decrease to about294.88. This matches controlled scroll state overshooting a native clamp that emits no further movement feedback. Recorded layouts preserve actual native offsets.

No repeated interleaving or wheel attempts followed these observations. Normal app mode remains separately blocked by Keychain saving; focus/scroll-tooltip and a complete visual icon-kind pass are not established. GUI was handed to the activity worker; proxy faults were cleared to `{}`.
