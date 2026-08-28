RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# FFI surface census

The machine-readable table in `ffi-surface-census.json` contains every exported Rust `extern "C"` function with name, source file, source line, and argument count.

## Exported functions

The current kernel has **212** `pub extern "C"`/`pub unsafe extern "C"` definitions and **212** `#[no_mangle]` attributes. The independent counts are:

```sh
rg -n 'pub (unsafe )?extern "C" fn' kernel/src --glob '*.rs' | wc -l
rg -n '#\[no_mangle\]' kernel/src --glob '*.rs' | wc -l
```

Totals by file are:

| File | Functions |
| --- | ---: |
| `kernel/src/ffi.rs` | 178 |
| `kernel/src/motion_gesture_ffi.rs` | 16 |
| `kernel/src/motion_outcome_ffi.rs` | 12 |
| `kernel/src/crash_capsule.rs` | 4 |
| `kernel/src/crash_retention.rs` | 2 |

Reproduce those totals from the machine table:

```sh
jq -r '.functions[].file' docs/kernel-refresh/w0a/ffi-surface-census.json | sort | uniq -c | sort -nr
```

The extraction command, including the argument counter used for every JSON row, is recorded verbatim in `ffi-surface-census.json` under `provenanceCommands.exportedFunctions`.

## Process-global callback registrations

WS-E's callback generations are all present:

| Family | Registration | Global storage |
| --- | --- | --- |
| native-control measure | `kernel/src/ffi.rs:1756` | `NATIVE_CONTROL_MEASURE_CALLBACK`, `kernel/src/ffi.rs:1716` |
| text measure v1 | `kernel/src/ffi.rs:2221` | `TEXT_MEASURE_CALLBACK`, `kernel/src/ffi.rs:1807` |
| text measure v2 | `kernel/src/ffi.rs:2236` | `TEXT_MEASURE_CALLBACK_V2`, `kernel/src/ffi.rs:1808` |
| text-measure runs v1 | `kernel/src/ffi.rs:2254` | `TEXT_MEASURE_RUNS_CALLBACK`, `kernel/src/ffi.rs:1902` |
| text-measure runs v2 | `kernel/src/ffi.rs:2270` | `TEXT_MEASURE_RUNS_CALLBACK_V2`, `kernel/src/ffi.rs:1903` |
| text-measure runs v3 | `kernel/src/ffi.rs:2285` | `TEXT_MEASURE_RUNS_CALLBACK_V3`, `kernel/src/ffi.rs:1904` |
| segment preparation | `kernel/src/ffi.rs:2403` | `TEXT_PREPARE_SEGMENTS_CALLBACK`, `kernel/src/ffi.rs:2382` |
| module action | `kernel/src/ffi.rs:4511` | `GLOBAL_ACTION_CALLBACK`, `kernel/src/modules/registry.rs:157` |
| module sync | `kernel/src/ffi.rs:4521` | `GLOBAL_SYNC_CALLBACK`, `kernel/src/modules/registry.rs:158` |

This is five text-measure generations—two uniform and three runs—plus the three other WS-E categories and native-control measure. Reproduce the setters with:

```sh
rg -n 'exact_set_(native_control_measure|text_measure|text_measure_runs|text_prepare_segments|module_action|module_sync)_callback' kernel/src/ffi.rs
```

## Opaque handles and status collapse

The hand-written header is **2,790 lines** and contains **10** exact `typedef void*` opaque handles. Provenance:

```sh
wc -l kernel/include/exact_kernel.h
rg -n '^typedef\s+void\s*\*' kernel/include/exact_kernel.h
rg -n '^typedef\s+void\s*\*' kernel/include/exact_kernel.h | wc -l
```

The handle typedefs are at header lines 22, 25, 1689, 1921, 1922, 2056, 2168, 2431, 2513, and 2650. The exact literal `Err(_) => -1` pattern occurs **21** times, all in `kernel/src/ffi.rs`:

```sh
rg -n 'Err\(_\)\s*=>\s*-1' kernel/src --glob '*.rs'
rg -n 'Err\(_\)\s*=>\s*-1' kernel/src --glob '*.rs' | wc -l
```

## Discrepancies

- RFC 0491 cites roughly 130 `Err(_) => -1` error-collapse sites. The exact literal grep it names finds 21 in this tree. Broader `return -1`, `None => -1`, and callback-failure conventions exist, but they are different syntax and are not silently substituted into this claim.
- The RFC's approximate 2.8K-line header claim matches the current 2,790-line file; this is rounding, not a substantive discrepancy.
