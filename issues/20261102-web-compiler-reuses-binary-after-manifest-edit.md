# The web compiler reuses its binary after a dependency manifest changes

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, build cache
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, a normal JavaScript web build can run a compiler built against an earlier dependency configuration after only a `Cargo.toml` or `Cargo.lock` edit. Changes to dependency versions or features then do not reach the compiler used for the build until a source edit or explicit compiler rebuild makes it stale.

`host/web-js/module.mjs:384` selects the local compiler using `fresh(COMPILER, depInfo)`. That check inspects source paths in dep-info, but omits manifests and the workspace lockfile. The adjacent `freshCompiler` helper at lines 369–375 includes those inputs, and the dev loop already uses it for copied compiler binaries. The normal `webCompiler()` path bypasses that stronger check; `host/web-js/build.mjs:182-185` runs the selected binary directly.

Run `bun /tmp/exact-review-web-compiler-fresh.mjs`. The fixture imports the actual, unmodified `module.mjs` with `CARGO_TARGET_DIR` pointing to its private temporary directory. It creates a compiler artifact and dep-info naming one dependency source, then makes only that dependency's manifest newer than the artifact. The result is:

```json
{"fresh":true,"freshCompiler":false,"selected":"stale fixture compiler"}
```

This exercises compiler selection with real filesystem timestamps. The artifact is a placeholder: no compiler process, Cargo build or changed dependency behavior was exercised. The temporary directory is removed by the script.

Use `freshCompiler(COMPILER)` before selecting the local binary, and apply the same manifest/lockfile freshness rule to other compiler reuse paths. Check dependency-manifest and lockfile-only changes with unchanged Rust sources.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

## Resolution

Use complete compiler freshness checks for the local compiler, explicit compiler override and prebuild reuse, including dependency manifests and the workspace lockfile.

## Verification

The compiler-reuse regression in `host/web-js/build.test.mjs` passes manifest-only and lockfile-only invalidation while unchanged Rust sources remain fresh.
