# Code review: LLP 1121 stage 11 A1, package designs in the compiler, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `901b41e3d5`.
- **Method:** one brief (sha256 `ce416d70e13dd505c8e0fc383d6d22881d0294b35d29e9e5d787ade82c1f8577`), blind to the other reviewer; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (3 BLOCKING, 7 SHOULD-FIX).
- **Disposition:** taken in the amended commit: overrides skip the package's `when`s, containment refused, the manifest's name, vendored packages found, base names refused, bindable carried, Cargo reruns on a missing design's directory, band bounds and messages, the other folds take packages. Kept, then ruled by theme2 13:06: the namespace collision refused only when both ship a design. Server (A2) noted.

Review of `901b41e3d5` found these issues:

- **BLOCKING — Reference checking produces false refusals and permits forbidden references.** [theme/packages.rs:205](contract/lower/src/theme/packages.rs:205): an app token named `light-dark` makes a package’s `light-dark(...)` fail; `face` similarly collides with literal `#face`. Conversely, app-only `reading.extra` bypasses checking through the namespace prefix, and app dials are absent from `own`. Validate resolved references rather than split words.

- **BLOCKING — Whole-token overrides happen too late.** [theme/packages.rs:232](contract/lower/src/theme/packages.rs:232): discarded package `when` branches and references are validated first. An app replacing `reading.card` completely still gets an unknown-axis refusal from the package’s old `when density=compact`. Replacement must remove those requirements.

- **BLOCKING — Package path containment is incomplete.** [resolve.rs:399](contract/cli/src/resolve.rs:399): `"tamber":"../outside.tamber"` silently disappears instead of being refused; an inside-looking symlink can load an outside file. Canonical containment is also missing for the default design path.

- **SHOULD-FIX — Namespace and identity use the import alias.** [sources/packages.rs:41](contract/cli/src/sources/packages.rs:41): `Package.name` comes from `use`, not `package.json`. Installing `@acme/reading` as `books` finds `reading.tamber` but demands `tamber books`; `home` likewise uses the wrong identity. Collision checking also excludes packages without designs, narrower than the supplied ruling.

- **SHOULD-FIX — Relative-only nested packages are missed.** [sources/packages.rs:21](contract/cli/src/sources/packages.rs:21): discovery uses the registered package list. A relative import into `./vendor/reading/index.contract` inherits its parent package’s origin, so the nested package’s manifest/design is never discovered—even though component identity recognizes that manifest.

- **SHOULD-FIX — Reserved base names can be declared by packages.** [theme/packages.rs:163](contract/lower/src/theme/packages.rs:163): a package named `space` can define `space.md`; likewise `radius`. The prefix check bypasses the prohibition on package definitions of base names.

- **SHOULD-FIX — D7 loses bindability without an app design.** [lower/lib.rs:1323](contract/lower/src/lib.rs:1323): the disk-derived bindability flag lives only in `file.design`. With package designs, no `app.tamber`, and code/hatches beside the app, otherwise unreferenced package tokens can be pruned incorrectly.

- **SHOULD-FIX — Adding a design does not reliably invalidate Cargo builds.** [cli/lib.rs:504](contract/cli/src/lib.rs:504): missing design paths enter `consulted`, but `rerun_if_changed` filters them out. Creating one alone can leave a successful build cached.

- **SHOULD-FIX — Line encoding has two holes.** [theme/packages.rs:160](contract/lower/src/theme/packages.rs:160): there is no bound enforcing fewer than 257 bands or lines below `BAND`; overflow reaches parameter/synthetic ranges and can misattribute diagnostics or panic. Also, [theme.rs:761](contract/lower/src/theme.rs:761) preserves embedded line numbers in messages: duplicate and ambiguous-`when` messages print encoded numbers even when their primary spans are correct.

- **SHOULD-FIX — Other compiler folds still omit packages.** [theme.rs:1135](contract/lower/src/theme.rs:1135) can suppress even unrelated app colour warnings when an app token references a package token. [sources.rs:618](contract/cli/src/sources.rs:618) similarly fails its standalone fold and drops app icon SVG dependencies from the graph.

- **SHOULD-FIX, A2 — The server disagrees with compilation.** [tamber_serve.rs:735](contract/cli/src/tamber_serve.rs:735) folds only the app and params. Valid app aliases of package tokens fail server reads/edits; package dependants are also absent from its live-update calculation.

What checks out: deterministic depth-first discovery, transitive package-name imports, relative files within one package, `exact.json` registration, missing explicitly named-file refusals, and ordinary token/`when`/value span decoding. Dev watchers handle absent paths safely. I found no current stale-`SOURCES` ordering bug: diagnostics are mapped synchronously, and threads have separate storage. Role overrides are correctly exempted; `var()` remains refused. App-only params handling and `edits.rs`’s isolated literal check should remain app-only.

The seven tests cover basic merging, pruning, several refusals, and one duplicate span. They miss the cases above; notably, the “params/components” test tests neither, and the reference test exercises neither roles nor base names.

No files changed. The permitted test command could not start: the sandbox denied creation of `/tmp/rv-s11a-target`.
