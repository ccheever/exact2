# Quick review: Harmony's empty test harnesses (637c994a60), Grok

**RIGHT.** The flags sit on the only Cargo target each host crate has, and they do not silence any tests.

`test = false` and `bench = false` match the carousel apps. `apps/harmony/app/web/Cargo.toml:9-12` and `apps/harmony/app/apple/Cargo.toml:9-12` put both keys on `[lib]`, same as `apps/carousel/web/Cargo.toml:10-14` and `apps/carousel/apple/Cargo.toml:10-14`. Harmony has no `src/main.rs` or `src/bin/`, so there is no `[[bin]]` to mark. `apps/harmony/app/linux/Cargo.toml:9-13` puts them on `[[bin]]` `app-linux` (`path = "src/main.rs"`), same as `apps/carousel/linux/Cargo.toml:13-17`. There is no `src/lib.rs`.

Nothing in those crates is a test. `web/src/lib.rs`, `apple/src/lib.rs`, `linux/src/main.rs`, and each `build.rs` have no `#[test]`, `#[cfg(test)]`, or fenced doc-test. The `include!` glue (`module.rs`, `logic.rs`, `host.rs`) is constants plus `configured!` / `host!`, which emit no tests. `test = false` does not turn off doctests (`doctest` is separate), and there are no code fences to run.

`scripts/caps.mjs:243-261` only requires those two keys on every lib/bin when `TEST_CODE` (`caps.mjs:45`) matches nothing. It does not match here, so `tests:unbuilt` does not apply, and `tests:empty-harness` is satisfied.
