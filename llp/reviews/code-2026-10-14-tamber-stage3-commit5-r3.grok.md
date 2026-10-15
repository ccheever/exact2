# Code review: LLP 1121 stage 3, commit 5, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `b6c9b55b441eb54327a6ad4e3f50f6a7242c6c159871757ad4b18268f976049e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: three medium, one low; Grok: two medium, one low.
- **Disposition:** applied without re-review

---

**Medium** — `scripts/tamber-commit.mjs:68`

`git cat-file --batch` reads object names one per line, but a clean path may contain a newline. The name is split, that path and every clean path after it are treated as missing (`atHead` stays null), and the first write records them as differing from HEAD. On this git, `HEAD:line\nbreak.tamber` then `HEAD:app.tamber` comes back as `HEAD:line missing` and `break.tamber missing`, so a normal `app.tamber` prepared later in the same batch is refused for the rest of the session. Status itself is fine (`-z`). Feed `git cat-file --batch -Z` with NUL-terminated names and parse NUL-terminated headers: a missing object is `HEAD:path missing\0` with no body; a blob is `<oid> SP blob SP <size> NUL <bytes> NUL`.

**Medium** — `scripts/tamber-commit.mjs:68`

That same `spawnSync` uses Node’s default 1 MiB stdout cap. The old code ran one `git show` per path, so the cap was per file; the batch is the sum of every clean blob. Over the cap, `spawnSync` throws `ERR_CHILD_PROCESS_STDIO_MAXBUFFER` before any fresh fact is stored. `prepare` has already incremented holds for paths another write still owns, and `write` has not installed `announce`, so those holds are never released and the POST never gets a reply. Hash through a stream, or set `maxBuffer` to the size you are willing to hold, and release in a `finally` if the read throws.

**Low** — `scripts/tamber-commit.mjs:62`

A staged rename with both sides in the path list is `R  <new>\0<old>\0` (the score is omitted and the order is reversed). The loop treats `<old>` as its own `XY path`, so `<old>` is stored under `<old>.slice(3)` and the real source is missing. A source that is gone is not written, so the false “clean” fact is released; a source that was put back is also reported as `??`, which this loop parses correctly. A design path that actually equals that slice is reported staged. If `xy` includes `R` or `C`, consume the next NUL field and record that path under the same `XY`.

Checked and sound:

- Quoted, spaced, untracked, and ignored paths: `-z` does not quote. `??` is “differed from HEAD”. An ignored untracked file is absent from status, `cat-file` says `missing`, and the write is refused because `atHead` is null.
- A missing HEAD path is one `… missing` line with no body, and the next object stays aligned. Present objects are `<oid> blob <size>`, so spaces in the path do not affect the size field.
- Holds match on success, conflict, 413, an already-dead server, a later reply after a client timeout, and `exit`/`fail`. `wrote` releases paths in `written`; `release` releases the rest.
- Rollback matches this git: `ls-files -s -z` fed back to `update-index -z --index-info`, and a new path removed with mode `0` and a zero name as long as `hash-object`’s (`40` fails in a SHA-256 repo, `64` works). A failed rollback is included in the error.
- Conflict `by` is set in the reply callback before the next stdout line. The first hello stays silent only while `known` is empty. A missing file on undo is a conflict (`read_files(true)` plus `stale`). Proposal exits go out as accepted, replaced (`replaces` / `replacedBy`), restored, or expired, and `close` clears the 30s timer.
