# Code review: the runner's substitute cache, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `46e65ae660`.
- **Method:** one brief (sha256 `37a3905b73380adb3427c7dd9e9a5e42ca61977637cec973bb2a98a06bf96034`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The cache in `46e65ae660` matches the plan. The key is the plan string id plus the host target, the shared `Arc<Json>` stays immutable, and a new plan boots a new runner with an empty cache. Both new tests pass.

The key is `(row.substitute.0, host_target())` in `runner/src/runner/modules.rs:562`. `substitute` is a `StrId` into that plan's string table, interned by content, so two modules with the same substitute text share one entry. That sharing is right: `this_target` (`runner/src/runner/modules/keyed.rs:31`) reads only the parsed JSON and `host_target()`. It drops `permission`, `choose`, and `permissionList` entries for other targets and writes `permissionElsewhere`. Locale, permission state, the module row, and a member answer do not enter the parse. A call's permission is applied afterward on an owned clone in `substitute_for` (`runner/src/runner/modules/admit.rs:750`). A member answer is matched against the cached object, and the value returned is `(*sub).clone()` (`runner/src/runner/modules/members.rs:80`). `value_of` still overlays `targets[host_target()]` onto a clone of `default` at read time (`runner/src/runner/modules.rs:782`), so that map stays whole in the cache and follows the target of the call. A different target is a different key.

`Json` is `serde_json::Value`. `Arc` has no `DerefMut`, and nothing calls `Arc::get_mut` or `Arc::make_mut` on this value. The only writes into a substitute object are `this_target`, before the `Arc` is built, and `substitute_for`, on the clone.

`Runner::plan` is never assigned. `boot` and `boot_carrying` both go through `boot_inner`, which sets `extensions: Default::default()` (`runner/src/runner.rs:835`). A dev reload that keeps slots and the clock is a new runner, so it parses the new plan's strings. Live batches refuse a structure change and do not rewrite module substitutes. `set_module_substitute` runs at bake, before a runner exists.

Each target string this runner has read keeps the substitutes parsed under it. Hosts set one platform name (`ios`, `linux`, `web`, and the same small set). Entries remain for every name read on that runner. That set is a handful of platform names.

`Runner` is already `!Send` (`Rc`, `std::cell::OnceCell`). The new `RefCell` matches that. `substitute` drops the map borrow before `this_target`, and `this_target` only reads `host_target` and edits the local value, so a miss does not `borrow_mut` under a live guard.

A substitute that fails `serde_json::from_str` still becomes `Null` via `unwrap_or_default`, and that `Null` is what is cached. The plan string does not change, so parsing it again would return the same `Null`.

Readers index through `Deref`. The two callers that need their own `Json` clone it first. Other behavior is the same parse plus `this_target`.

`cargo test -p exact-runner --lib runner::modules::substitute_tests` passed both tests. They go red without the early return. `substitute_polls` calls `substitute` twice per module (`samples_of`, then `counts`), so the setup loop alone is 4 parses against `read <= 2`. A scroll also advances: `launch_clock` starts at 0 and the events are at 16 and up, and `listed_due`, `can_lock`, and `release_sampled` read every module again. The other test boots two runners, each with a fresh `Extensions`, and `5000` and `250` stay on their own runner.

No findings.

VERDICT: APPROVE
