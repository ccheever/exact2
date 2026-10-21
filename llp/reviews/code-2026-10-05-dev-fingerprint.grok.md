# Code review: the web dev driver compares used files by fingerprint (host/web/src/dev.rs), 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, a detached worktree at `be1669d21`.
- **Method:** the shared brief (sha256 `a2dbbc65abad33c2ec932759814adc7ff2d21966e84dda16a0e0511c4fcc5bb7`); one round; blind. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; opening sentences are progress narration.
- **Verdict:** LAND.
- **Disposition:** No findings.

---

I'll review only commit `6fed6f8d5` against its parent, read-only, and start from the diff plus the LLP 1091 context around the fingerprint change.The diff is a fingerprint-only compare in `poll`. Next I’ll check what `stamp_of` already covers and whether LLP 1091 or the dev tests still depend on the modification time.LAND

No findings.

The change is correct and minimal. `poll` still re-reads each used file every look; `stamp_of` hashes those bytes, the canonical path, and on Unix the device and inode. Modification time was only an extra inequality, so an identical rewrite was a change. Nothing else the time caught is left uncovered: a copy that keeps its timestamps still differs in bytes (LLP 1091 round 9), and a link or junction retargeted at a byte-identical twin still differs in canonical path or device/inode (rounds 10–11), including the root since `30566fa2a`. `uses` is private to `host/web/src/dev.rs`; the only reader was this comparison. `saved_ms` still comes from the root and `surfaces.json` stamps. The root's own identical-bytes check then returns `None`, which is what `a_save_becomes_a_plan_and_an_identical_save_is_nothing` asserts. The other `exact-web` dev tests still rebuild on a real edit, a preserved `(mtime, length)` after a failed compile, and a surfaces change. Skipping the identical rebuild matches the dev-restart budget. Tests were traced, not run.
