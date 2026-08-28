RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# Discrepancies found during the W0-A census

1. RFC 0491 describes `Node` as approximately 570 bytes; the target used by the required integration pin measures **576 bytes**. Reproduce the measurement with:

   ```sh
   cargo test -p exact-kernel --test w0a_layout_pins
   ```

2. RFC 0491's **54** string-literal-probe statement is reproducible only under the syntax-probe definition captured in `prop-model-census.json`: 42 same-line `props.get("...")` occurrences, one same-line `contains_key`, seven `Kernel::set_prop` dynamic-key comparisons, and four boolean-string parses. It is neither 54 unique source lines nor 54 unique prop names. Reproduce the category totals with the string-probe extraction entry in that JSON's `provenanceCommands` array.

3. `kernel/src/lib.rs` probes `svgImageRasterReceipt`, but `kernel/src/protocol/prop_decoder.rs` has no wire ID for it. It may be host/internal-only, but that classification is not expressed by the current prop table. Reproduce with:

   ```sh
   rg -n 'svgImageRasterReceipt' kernel/src/lib.rs
   rg -n 'svgImageRasterReceipt' kernel/src/protocol/prop_decoder.rs
   ```

4. RFC 0491 cites roughly **130** literal `Err(_) => -1` status collapses. The exact grep named by this pack finds **21**, all in `kernel/src/ffi.rs`; broader `return -1`, `None => -1`, callback failure, and `unwrap_or(-1)` conventions exist but are different syntax. Reproduce with:

   ```sh
   rg -n 'Err\(_\)\s*=>\s*-1' kernel/src --glob '*.rs' | wc -l
   rg -n 'Err\(_\)\s*=>\s*-1' kernel/src --glob '*.rs' | cut -d: -f1 | sort -u
   ```

5. RFC 0491 calls the Rust motion-clock registry roughly 245 lines. The broad contiguous support block `MotionClockConsumerKind` through `MotionClockRegistry` is **204** lines (`kernel/src/motion.rs:1512-1715`), while the registry type/implementation itself is **125** lines (`:1591-1715`). Reproduce with:

   ```sh
   sed -n '1512,1715p' kernel/src/motion.rs | wc -l
   sed -n '1591,1715p' kernel/src/motion.rs | wc -l
   ```

6. RFC 0491's WS-F population-zero direction includes the v1 `InteractionArena` and direct-setter write path, but both remain live migration candidates. The interactive-navigation FFI owns an `InteractionArena`, and the Apple wrapper still calls seven direct setters. Reproduce with:

   ```sh
   rg -n 'arena: crate::motion::InteractionArena|InteractionArena::new' kernel/src/ffi.rs
   rg -n 'exact_set_(style|children|node_text)|exact_add_root|exact_compute_layout' ios/ExactApp/ExactApp/Kernel/ExactKernel.swift
   ```

7. RFC 0491 says “dead enum variants” without naming the complete set. The census can prove `MotionDescriptorKind::Driver` is inventory-reserved and rejected by the snapshot validator, but cannot prove an unnamed remainder. Reproduce with:

   ```sh
   rg -n 'MotionDescriptorKind::Driver' kernel/src/motion_transport.rs
   ```

8. RFC 0491's in-process-host discussion protects `HostInterpositionFrame`, and the requested census groups Windows and TUI under that seam. In the current tree only TUI uses `HostInterpositionFrame`; Windows parses with `OpParser` and calls `execute_ops` directly. Both are in-process Rust consumers, but their current decode seams differ. Reproduce with:

   ```sh
   rg -n 'HostInterpositionFrame' packages/exact-host-windows packages/exact-host-tui --glob '*.rs'
   sed -n '347,365p' packages/exact-host-windows/src/scene.rs
   ```

9. The generated recovery authority assigns list-model the target tree-bound rollback class while recording a current fail-open divergence: malformed staging is silently dropped and dispatch returns success while other tree mutations can commit. This is not a contradiction in the generated authority; it is a current-tree-versus-target discrepancy the consumer join must preserve. Reproduce with:

   ```sh
   jq '.rows[] | select(.family == "list-model") | .knownDivergence' tests/protocol/exwf/recovery-classes.json
   sed -n '421,490p' kernel/src/native_list.rs
   ```

10. RFC 0491 calls the Rust `MotionClockRegistry` dead, and repository production-source search does find zero callers, but `kernel/src/lib.rs` exposes the `motion` module and both `MotionClockRegistry` and `MotionPager` are public Rust types. Their repo-local population is zero; whole-ecosystem/rlib deadness is not proven and the WS-F rows therefore remain `verified: false`. Reproduce with:

   ```sh
   sed -n '68p' kernel/src/lib.rs
   rg -n '^pub struct (MotionClockRegistry|MotionPager)' kernel/src/motion.rs
   rg -n '\bMotionClockRegistry\b|\bMotionPager\b' kernel packages contract-native ios android vendor --glob '*.rs' --glob '*.swift'
   ```

The hand-written header is 2,790 lines against RFC 0491's approximate “2.8K,” which is ordinary rounding rather than a substantive discrepancy:

```sh
wc -l kernel/include/exact_kernel.h
```
