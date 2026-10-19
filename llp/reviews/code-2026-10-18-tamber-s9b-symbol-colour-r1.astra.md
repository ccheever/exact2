# Review round 1, LLP 1121 stage 9 B (symbol images take their written colour), Astra (gpt-6-astra, max)

**WRONG**

1. **BLOCKING — [host/web-js/src/style.rs:359](/tmp/rv-s9b-r1/host/web-js/src/style.rs:359): Bound symbol sources miss written colour.**  
   Under `column color="red"`, `image icon` stays accent-coloured when `icon` comes from state or data. The expression contains no literal `symbol:` string, and `symbols.js` never repairs the missing marker. The new `named-elsewhere=false` test enshrines this failure. **Fix:** determine symbol status at runtime, or mark colour eligibility independently of source—the CSS already requires `data-symbol-path`. Change the test to expect written colour.

2. **BLOCKING — [host/web-js/src/style.rs:376](/tmp/rv-s9b-r1/host/web-js/src/style.rs:376): Conditional colour remains “written” after removal.**  
   An ancestor with `color=(on ? "red" : none)` permanently marks its symbol descendants. When `on` becomes false, they draw in the inherited default label colour instead of the unsaid accent. Contrary to the comment, the runner clears the written bit for `Value::Option(None)`; CSS-wide clears retain it. **Fix:** update colour provenance with binding values and reconsider the nearest setter when a declaration disappears. Test both directions of this transition.

3. **BLOCKING — [runner/src/instance/document.rs:125](/tmp/rv-s9b-r1/runner/src/instance/document.rs:125): Kernel-free documents lose authored clears.**  
   `column color="inherit"` containing a symbol is marked by the kernel, but not by `DocTree`: `fold()` discards the declaration at line 399 without preserving `written`. Consequently, prerendered and live documents choose different symbol colours. **Fix:** preserve `binding.written` for CSS-wide clears while leaving `none` unwritten. Add this case to the document-parity test.

4. **BLOCKING — [kernel/src/node.rs:65](/tmp/rv-s9b-r1/kernel/src/node.rs:65): The control-face exclusion misses projected segments.**  
   A symbol-only `Pressable` tab can become a native segment through `segmentFace.image`; its parent is not `Control`. With inherited red and hierarchical rendering, the new `symbolInk` bakes red into `icon.image`, which [SegmentsIOS.swift:237](/tmp/rv-s9b-r1/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:237) rasterizes as `.alwaysOriginal`. Thus a native segment consumes page ink despite D9 and the documentation’s contrary claim. **Fix:** account for projection ownership, or construct segment symbols independently of page-image ink. Test projected segments beside page symbols under the same authored colour.

5. **SHOULD-FIX — [kernel/src/node.rs:67](/tmp/rv-s9b-r1/kernel/src/node.rs:67): Reparenting can leave the marker stale.**  
   Move a retained symbol between ancestors supplying identical colour values but different written bits. `symbol_color_said()` changes, but [txn.rs:1317](/tmp/rv-s9b-r1/kernel/src/txn.rs:1317) compares only inherited values, so the image need not be touched or resent. **Fix:** include colour provenance in topology-change invalidation and test equal-colour moves in both directions.

Caps passed, including `glue.js` at 1,499 lines. In-memory JS probes confirmed the missing/stale markers. Cargo tests could not start because the read-only sandbox denied creating the isolated target directory.
