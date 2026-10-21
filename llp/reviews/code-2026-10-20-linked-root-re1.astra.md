# Narrow re-check: a linked root (112e6c714a), Astra

**RIGHT WITH CHANGES** — NITs only; no BLOCKING findings.

The functional fixes are sound. `load_partial`, `graph`, and `use_fixes`/`root_loader` preserve the supplied path in `Loader::root_given`; only source 0 can use that fallback. Graph paths, import keys, and caching remain canonical. Empty parents become `"."`.

Non-root identity and package refusals remain intact: `home()` retains precedence, unnamed external packages are refused, and relative imports undergo canonical containment checks. The extended test explicitly asserts `contract-use-path` for an import through an external link.

- **NIT — documentation still omits package precedence.** [contract-grammar.md:209](/tmp/rv-unnamed-astra/docs/contract-grammar.md:209) and [1121-tamber.rfc.md:1271](/tmp/rv-unnamed-astra/llp/1121-tamber.rfc.md:1271) now correctly limit the exception to a root file, but still unconditionally call it the app’s own. Qualify this as the fallback when canonical naming finds no home: a root linked into a named package retains that package’s identity.
- **NIT — formatter regression coverage is missing.** [tamber_params_tools.rs:506](/tmp/rv-unnamed-astra/contract/cli/tests/it/tamber_params_tools.rs:506) exercises bare-name `build` and `sources`, but not `fmt --uses`, despite its separate loader constructor being one of the original failures. Add that invocation; also assert the graph’s root path equals the canonical target.

Read-only source review; no builds or tests run.
