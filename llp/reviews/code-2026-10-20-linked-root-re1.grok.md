# Narrow re-check: a linked root (112e6c714a), Grok

**RIGHT**

No findings.

`Loader::root_given` is the path the root was named by. `load_source` applies `linked_home` to it for source 0 only, after `home` on the canonical path returns nothing (`sources.rs:746`). `graph` (`sources.rs:551`) and `root_loader` (`sources/uses.rs:412`, the `contract fmt --uses` loader) store `canonicalize` in `paths[0]`, so `contract sources` stays canonical while the identity is the given path.

A bare name’s parent is empty. `linked_home` drops that and canonicalizes `.` (`sources.rs:348`), the same rule as `app_root` (`lib.rs:727`). From the app directory, `tabs.contract` is named `tabs.contract`.

`docs/contract-grammar.md:209` and `llp/1121-tamber.rfc.md:1271` limit the exception to a root file and say a file a `use` reaches is the target’s. The test (`tamber_params_tools.rs:491`) asserts `source_graph` has no errors, bare `build` and `sources` exit 0, and `use ./card.contract` through a link out of the app is `contract-use-path`. `resolve` rejects that key before `linked_home` (`resolve.rs:161`).

A later source keeps `resolved.key` and is named by `home` alone. An unnamed package is refused from `home` in `link` (`sources.rs:796`). `linked_home` returns nothing for `Origin::Package`.

Read-only; the suite was not run.
