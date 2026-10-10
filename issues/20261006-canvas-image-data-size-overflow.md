# Check ImageData dimensions and allocation size before constructing its pixel buffer

**Status:** Open
**Systems:** Canvas 2D, Rust recorder
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1056 D3 and D9, canvas/src/context/image.rs:195

`ImageData::new_with_sw` returns `Result<ImageData, DomException>`, but computes `sw as usize * sh as usize * 4` directly inside `vec![0; ...]`. It checks zero dimensions only. The arithmetic and allocation can panic or abort instead of returning the recorder's promised exception.

Verified against the built exact-canvas crate:
```rust
std::panic::catch_unwind(|| ImageData::new_with_sw(u32::MAX, u32::MAX))
// Err: attempt to multiply with overflow, image.rs:202
```
The same call in Chrome, `new ImageData(4294967295, 4294967295)`, throws `IndexSizeError` because the requested size exceeds its supported range. On a 32-bit wasm target, products overflow at much smaller dimensions. An allocation failure in native Rust can terminate the application.

Use checked dimension/product conversion and reject unsupported sizes with the appropriate exception before allocation. Handle recoverable reservation failure through the same result. Ensure `Context2d::create_image_data_*` and all constructor overloads use the checked path. This is a constructor correctness issue, not a request to add new arbitrary state/path limits that D4 explicitly removed.

Acceptance: zero, oversized and overflowing dimensions return a named error without panic, abort or wrapped undersized storage. Ordinary byte lengths are exactly width × height × 4 on native and wasm, and the existing Canvas recorder corpus still passes.
