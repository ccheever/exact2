# Code review: a shared helper ending a test's children and what they started (scripts/test-children.mjs), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `d7f057243`.
- **Method:** one brief (sha256 `f7539803baedc84eddf266a53b74e571aa52ce9645bb242b0d10ac899b9eab8a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two should-fix (a failed or partial ps; the 400 ms waits) and two nits (lstart to the second; ps per child).
- **Disposition:** taken in e2dc1362cd (LC_ALL=C and one shared look; whole-second lstart said).

1. **SHOULD-FIX** — `scripts/test-children.mjs:23` (wipe at `:39`, kill at `:82`). `table()` ignores `ps` failing, timing out, or returning a partial listing. `note()` then deletes every known pid whose row is missing, and `remaining()` treats an empty table as "nothing left," so the `SIGKILL` set is empty. A `SIGTERM`-deaf descendant that was only known because an earlier look saw it, or one still running when `ps` fails during the grace, is left alive — the original leak. Fix: if `ps` did not exit 0, keep the previous `known` map and `SIGKILL` those pids anyway; delete a pid only when this snapshot shows it under a different `lstart`.

2. **SHOULD-FIX** — `scripts/test-children.test.mjs:28` and the same sleep in the nested file at `:45`. Both waits assume a 250 ms `ps -A` has finished within 400 ms of the grandchild's pid line. Under load that snapshot is still running, `known` does not contain the grandchild, and `stopAll` never signals it, so the async-lane test fails. Fix: poll until `child.known` has that pid (and its stored `lstart`), then call `stopAll`.

3. **NIT** — `scripts/test-children.mjs:64`. `lstart` is `%c`, whole seconds. A pid reused by an unrelated process in that same second still matches and is signaled. The pid space has to wrap inside that second, which does not happen on this lane. A later second does not match. Fix only if you want it closed: compare a start time finer than one second, and keep forcing one locale so the string cannot change underneath a live process.

4. **NIT** — `scripts/test-children.mjs:50`. Every owned child still runs `ps -A` every 250 ms, and the grace loop runs it every 50 ms. One dev loop is cheap; several overlapping ones are not. A group or pid-only probe, or one shared snapshot for all owned children, is enough.
