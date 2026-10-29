# Code review: the runner's substitute cache, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `4025946c87`.
- **Method:** one brief (sha256 `85b44e35ecb5492bbfa21f0ec862fcd4691aba20e9bbb6d0d69d175c6e3bd694`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The round-1 scroll and cache-growth fixes hold, and the target-specific parse is not shared across a target change. One test-isolation issue remains.

`this_target` still runs before the value is stored (`runner/src/runner/modules.rs:569-572`). The cached `Arc<Json>` is the document after permissions, `choose`, and `permissionList` have been filtered for the current host target. It is not a target-independent parse with the target applied on read. That is safe for two reasons. `this_target` (`runner/src/runner/modules/keyed.rs:31-57`) depends only on that JSON and `host_target()`, not on which module row it is, and plan strings are interned (`plan/src/builder.rs:128-135`), so two rows with the same substitute text are the same `StrId` and the same filtered document. Fields that still vary by target (`targets`, `allowOn`, `requires`, `denyOn`, `fill.on`) are left in the object and indexed with the live `host_target()` at the call.

`host_target()` (`runner/src/runner/modules/pictures.rs:159-164`) reads `pictures.target` or else `ceremony::target()`, which is a compile-time `cfg` value (`runner/src/runner/modules/ceremony.rs:45-60`). The only writes of `pictures.target` are the two assignments in `set_module_target`. Boot does not set it. Agent ops do not set it. The host paths (Linux `serve_packages`, the Android canvas, the terminal host, the web renderer) all call `set_module_target`. When the new string differs, the target is stored and the map is cleared on the next line, before `announce`. `set_module_target` takes `&mut self`, `Runner` contains a `RefCell` and the crate forbids unsafe, so nothing else can read the map in that window. The equal-string early return does not clear, and does not need to: `host_target()` already returned that same string, so the cached filter is still the one `this_target` would compute.

The map is one `Arc<Json>` per distinct substitute string since the last target change, so it stays within the plan. Callers that change a substitute (`substitute_for`, `member_answering`) clone the `Json` first.

The scroll test dispatches a scroll whose handler commits: no error, a non-empty receipt, and the `scrolls` slot changes (`runner/src/runner/modules/substitute_tests.rs:104-107`). The 38 later dispatches are the ones measured. Each `dispatch_at` reads substitutes (`pump_presence` → `local_auth`, and `apply` → `retire_holds` → `release_sampled` → `substitute_polls`), so those dispatches parse again with the cache off. The 50-target test is what fails if the clear is skipped: `permission` is present only for `target-7`.

VERDICT: APPROVE

1. MINOR — `runner/src/runner/modules/substitute_tests.rs:123` (counter at `runner/src/runner/modules.rs:93`). `SUBSTITUTE_PARSES` is a thread-local incremented by every `substitute()` under `cfg(test)` and never reset. The runner unit-test binary reuses threads, and `a_changing_target_keeps_one_substitute_a_module_read_for_it` alone parses once per target. If it runs first on that thread, `read <= 2` fails even though the cache is working. The later `parses() == read` check is unaffected. Zero the cell at the start of the scroll test.
