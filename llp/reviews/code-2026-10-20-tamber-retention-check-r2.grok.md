# Code review: LLP 1121 stage 12 B1 follow-up, the background whole build, round 2 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `301a831244`.
- **Method:** one brief (sha256 `b05961431317a59a7803520e35dcb3a10f06badbee961f1f9fc602b47a83bdc0`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 SHOULD-FIX, 7 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's "Round 2").

## Round 1

Closed:

- A failure is stored on the retention key whenever the build's own before and after fingerprints match (`tamber_check.rs:232-236`). A lost build changes nothing and the next answer sends the files as they are now (`:276-277`). The two-write test reaches `rebuild`.
- The single `next` slot is gone. The worker keeps the newest job waiting (`:178-182`). Asking for the running job again does not leave an older one queued.
- A panic in `compile_path_all` is a failure (`:150-151`). A dead worker is dropped (`:216-224`) and the next unverified or failed answer starts another.
- An open scrub frame calls `known` under the key it opened with (`tamber_scrub.rs:72-78`, `:111-114`). The test goes `live`, then `rebuild` after the failure line.
- A completed `(key, fingerprint, false)` is not built again (`tamber_check.rs:274-275`). The worker pulls the next job itself, so a queued check proceeds while idle.
- The log line is written after the result is sent (`:184-187`). The protocol sentence now includes the background whole build (`docs/tamber-protocol.md:683-685`).

## SHOULD-FIX

`tamber_check.rs:113-126`, with `tamber_check.rs:274-275` and `contract/cli/src/strings.rs:55-69`. The directory half of the fingerprint uses `symlink_metadata` and does not descend a symlink. `strings::load` enumerates `strings/` and reads each table with `read_dir` and `std::fs::read`, which follow links, and it does not require the target to stay inside the app. Fonts and sounds do (`contract/lower/src/fonts.rs:142-159`, `contract/lower/src/sounds.rs:116-123`), so an in-app target is also a real directory entry and an ordinary edit changes its length or mtime. A strings table reached only through a link can change with the link's length and mtime unchanged. Both fingerprints still match, the failure is remembered, and `known` returns on `Some(false)` without sending again. The fixing edit never clears the key.

Contract sources, designs, `app.json`, consulted manifests, and icon SVGs are content digests, re-read by the worker before and after. That is the gate the ruling names. The stamp covers the rest only as real directory entries.

## NIT

- `tamber_check.rs:241-243`. `done` keeps 16 completions for the whole server. Dropping a failure entry while the verdict stays `Failed` builds those files once more. The commit message's "never built twice" holds inside that window.
- `tamber_check.rs:229-231`. A result equal to the last job sent clears `sent` even when that same job is still queued behind the one that just finished. The extra build records the same fingerprint. `the_newest_job_is_built_and_a_failure_under_the_key_stands` flips the design and calls `known` in sequence; it never has A in flight, B queued, and A sent again.
- `tamber_check.rs:196-198` and `:266-267`. Past 64 keys, `made` drops every verdict that is not `Failed`. A missing key answers live and sends nothing. A lost check of an unverified key that was dropped is not asked again. The key `made` is inserting is put back, and failed keys stay.
- `docs/tamber-protocol.md:684-685`. The sentence says the closure lasts until one of the files as they are passes. Clearing requires the whole fingerprint (`tamber_check.rs:270-273`). The bench numbers exist only in the commit message; this review did not remeasure them.
- `tamber_scrub.rs:8`. A frame now calls `known`, which walks and, while the key is unverified or failed, stats the app tree on the answer path (`tamber_serve.rs:459-465`).
- `tamber_check.rs:302-306`. If the thread cannot be spawned, that one answer runs the whole build in place.
- Tests. `a_check_raced_by_an_edit_mid_build_is_dropped` stops at the `lost` line. `a_lost_check_is_asked_for_again` fails if the ink write misses the 800 ms delay, including when the check has already failed and the answers are already `rebuild`. `a_value_edit_while_broken_is_rebuild_and_a_fixing_one_returns_to_live` requires the propose right after `zero = 2` to be `rebuild`, which is false once that propose has polled the pass. `a_failure_of_files_since_moved_still_marks_the_key` accepts a later failure of the files as they are now.
- A tree that changes and changes back between the two fingerprints is recorded for the endpoint bytes. A failure stored that way is not rebuilt until the fingerprint changes.

## What checks out

Once a failure has been polled, the next `propose` and every later scrub frame under that key are `rebuild`, until a passing build of the current fingerprint clears it. A lost result, a pass of some other fingerprint, and a superseded job do not clear. Failure is stored on the retention key, so an unrelated value edit while that key is failed stays `rebuild`. The answer that starts the check still comes from the retention build: `synthetic` discards the `known` result after `made` (`tamber_serve.rs:451-453`), and an unverified key answers live. That is the breaking write, and the live window until the worker reports.

The same stable fingerprint is built once and then remembered. Lost builds are sent again; a stable tree produces one `Built` result and stops. `compile_path_all` returns a plan and does not write the app tree, so a build does not make its own after-fingerprint miss.

One checker, one thread, for the server's life. The `JoinHandle` is dropped. Answers use `try_recv` and an unbounded send. Close returns because the process exits without joining; the 30 s delay against a 5 s close covers that. A result that arrives after the channel is dropped is unread (`:184-185`).

The worker is a fresh thread. Retention stop, the planned source text, and the compiler's theme, icon, and package state are thread-local. The kernel intern tables are mutexes held for the intern (`kernel/src/style/calc/mod.rs:321`, `kernel/src/style/handles.rs:54`). A retention build on the answer thread and a whole build on the worker share those tables and nothing else. In the dev profile `catch_unwind` records a panic as a failure. Release builds abort on panic, as an in-place build on main does.

`Walk::inputs` (`tamber_walk.rs:90-95`) and `digest_of` (`:50-54`) are the walk's files by content: the root, `app.json`, Contract sources, designs, consulted manifests, icon SVGs. Strings, font bytes, sound bytes, and `.shells` are the directory stamp, with the symlink hole above. After a pass, `known` returns on the verdict alone (`tamber_check.rs:266-267`); a later value edit that keeps the key stays live. `QUEUE.md` records that as main's behavior too.
