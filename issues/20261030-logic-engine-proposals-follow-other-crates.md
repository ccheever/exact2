# exact-logic's wasm engine accepts SIMD when another crate of the app enables wasmi's `simd`

**Status:** Open
**Systems:** Logic (Rust wasm executor, `logic/src/wasm.rs`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's timeline lane
**Date:** 2026-10-30

**Problem.** exact-logic depends on wasmi without the `simd` feature (`logic/Cargo.toml:16`), so built alone its engine refuses a Rust logic module that uses SIMD. When another crate in the same app enables that feature, Cargo unifies features, and exact-logic's engine then accepts SIMD and relaxed-SIMD modules. Nothing reports the change. The T3 Code clone (`daehyeon/t3-code-main`) does this: its data crate runs `@pierre/highlights`' lexers with wasmi and `simd` (`examples/t3-code/data/Cargo.toml:26`), and `t3-code-macos` links that crate and exact-logic together.

**Cause.** `logic/src/wasm.rs:39` starts from `Config::default()` and sets fuel, the start function, multi-memory and stack limits, but no SIMD proposal. wasmi 2.0's default features set SIMD and relaxed SIMD to `cfg!(feature = "simd")` (`wasmi-2.0.0/src/engine/config.rs:94-95`), so they are on whenever any crate compiles the feature in. `Config::wasm_simd` and `wasm_relaxed_simd` exist only with that feature (`config.rs:323,334`), so exact-logic cannot turn them off while it leaves the feature off. `memory64` (`config.rs:92`) follows its feature the same way.

**Reproduction.** Add a data crate that depends on wasmi with `features = ["simd"]` to an app with a Rust logic module, and give that module a SIMD instruction: `load` accepts the module. Without the data crate, `load` refuses it.

**Fix.** exact-logic sets each proposal it accepts explicitly, so another crate's features cannot widen them. For example, depend on wasmi with `simd` and call `.wasm_simd(false).wasm_relaxed_simd(false)` (and the same for `memory64`). Or check the module's features against an allow-list before `Module::new`.

**Acceptance.** A test in `logic` loads a module with one SIMD instruction and gets a refusal, also when the test crate enables wasmi's `simd` feature.
