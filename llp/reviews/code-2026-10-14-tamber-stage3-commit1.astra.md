# Code review: LLP 1121 stage 3, commit 1 (tamber fmt), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `73a51d422820424a80b473a9d92b29b76e55a189c74473e84cfc20b2da646358`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: one high, three medium.
- **Disposition:** all fixed in round 1

---

No high-severity findings. I found two medium and two low issues.

1. **Medium — formatting is not idempotent after a final comment.** [tamber/src/fmt.rs:86](/tmp/rv-s3c1/tamber/src/fmt.rs:86)  
   For input ending `  ink = #fff\n// tail\n\n`, the first pass removes the final blank line. The second pass inserts a blank **before** `// tail`. The lookahead sees a blank on pass one and EOF on pass two, classifying the comment differently. Thus `fmt` followed by `fmt --check` can fail.  
   **Fix:** decide comment grouping after normalizing trailing blank items. Add regression cases for final comment runs followed by blank lines, including CRLF.

2. **Medium — removing one `when` line changes neighboring comments and whitespace.** [tamber/src/fmt.rs:65](/tmp/rv-s3c1/tamber/src/fmt.rs:65)  
   In an already formatted design, removing the final `when` from:
   ```text
     ink = #fff
       // darker variant
       when tone=hard: #000
   ```
   causes its comment to move to column zero and gains a preceding blank line. `unset-token-when` therefore produces additional changes beyond the removed line. Similarly, block-ending comments can become attached to the following block.  
   **Fix:** preserve comments’ block/token placement rather than assigning every comment the next code line’s indentation. Test comment placement across insertion and removal of `when` lines.

3. **Low — the header has multiple accepted layouts.** [tamber/src/fmt.rs:52](/tmp/rv-s3c1/tamber/src/fmt.rs:52)  
   Both `tamber t` and `tamber    t` survive formatting unchanged because top-level text is copied verbatim. This misses the promised canonical layout.  
   **Fix:** print the header as `tamber {parsed_name}`, retaining its trailing comment.

4. **Low — the corpus gate silently skips required fixtures.** [tamber/tests/fmt.rs:36](/tmp/rv-s3c1/tamber/tests/fmt.rs:36)  
   Any formatting error skips that file; the aggregate `formatted > 10` assertion still passes if the Bluesky fixture is rejected. A checker error also silently disables its IR comparison.  
   **Fix:** distinguish intentional refusal fixtures explicitly and require every known-valid corpus file and named app fixture to format and check successfully.

Read-only validation: caps, all three prebuilt formatter tests for this commit, and CLI help/`--check`/exit-code checks passed. The comment counterexamples were checked with an in-memory transcription of the layout pass. No rebuilds or write-path tests were run; no files were changed.
