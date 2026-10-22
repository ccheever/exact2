# Quick review: a file linked into the app (c24e871110), Astra

**RIGHT WITH CHANGES**

1. **BLOCKING — bare filenames still fail.** [sources.rs:345](/tmp/rv-unnamed-astra/contract/cli/src/sources.rs:345): `Path::parent()` for `"tabs.contract"` returns an empty path; canonicalizing it fails. `app_root()` already treats that case as `"."`, but `linked_home()` does not. Repro: from `host/web-js/conformance`, run `contract build tabs.contract`; it still produces `contract-package-unnamed`, whereas `contract build ./tabs.contract` takes the fallback. Normalize the empty parent to `"."`. The new absolute-path test misses this ordinary invocation.

2. **BLOCKING — source discovery still rejects the newly accepted roots.** [sources.rs:736](/tmp/rv-unnamed-astra/contract/cli/src/sources.rs:736) assumes `sources.paths[0]` preserves the supplied path, but `graph()` initializes it with `root_key` at [sources.rs:550](/tmp/rv-unnamed-astra/contract/cli/src/sources.rs:550). Repro: `contract sources host/web-js/conformance/tabs.contract` still returns `contract-package-unnamed` and exits 1, although `build` accepts that path. Discovery stops before following imports, also compromising dependency watching for linked roots with dependencies. Preserve the supplied root path for identity calculation while retaining canonical graph output. Extend the regression test to assert `source_graph(&path).errors.is_empty()`.

3. **NIT — documentation overstates the exception.** [contract-grammar.md:209](/tmp/rv-unnamed-astra/docs/contract-grammar.md:209) and [1121-tamber.rfc.md:1271](/tmp/rv-unnamed-astra/llp/1121-tamber.rfc.md:1271) describe any file linked into the app. The implementation effectively admits a **root-file link when canonical naming finds no home**. Repro: an app importing `./card.contract`, symlinked to an external non-package file, still receives `contract-use-path`. A named-package target also retains its package identity through `home()`. Qualify both statements accordingly.

The core fallback is sound and small: it strips a canonical app prefix, so it does not introduce machine-directory identities. Non-root imports store canonical `resolved.key` values at `sources.rs:830`; relative escape checks run earlier, package refusals remain intact, and canonical caching prevents duplicate identities through alternate import routes. The supplied-path exception therefore does not introduce import-order dependence.

The 197-root comparison and three byte-identical plans are plausible: successful `home()` results are unchanged, and the three links have their targets’ basenames and contents. Those claims do not cover the two invocation gaps above. The test verifies the intended absolute-root case, but is insufficient alone.

Read-only review; no builds or compiler tests run.
