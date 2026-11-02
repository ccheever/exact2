# Code review: feedback nudges and one-line notes, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `a1ee449658`.
- **Method:** one brief (sha256 `7800da7566c084671d8881592dd7d092ee38606c0eb8535e5641cfa816044e7c`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** every finding is addressed in `4536dc5e10` (Astra's) and `2e7a214d38` (Grok's); round 2 reviews `2e7a214d38`.

- **BLOCKER — [scripts/feedback-note.mjs:59](/Users/admin/x2scratch/fb1-rv-astra/scripts/feedback-note.mjs:59): Arbitrary stderr can leak source and secrets.** `errorTail()` removes selected gutter formats, then accepts everything else. Read-only probes confirmed that unnumbered source, `Authorization: Bearer …`, and `--password …` survive both filtering and redaction. This is reachable through actual test output: `agent-test.mjs:1083` includes the app’s console logs in stderr. `always` then sends these automatically. **Fix:** export explicitly safe diagnostic fields, excluding runtime values, source excerpts and echoed arguments; sanitize before truncation. Add regressions covering real diagnostic formats, console logs, credentials and long lines whose gutter is truncated.

- **MAJOR — [game/new.mjs:903](/Users/admin/x2scratch/fb1-rv-astra/game/new.mjs:903): `never` projects still carry required diary instructions.** Both app and game templates unconditionally insert the instruction to record feedback. Refusing the eventual command does not satisfy LLP 1116 D3’s requirement that opted-out agents carry no diary instructions. `docs/diary.md:21` reinforces this with “whatever the answer,” contradicting its earlier `never` exception. **Fix:** keep the generated section as a pointer to `feedback status`, and emit recording instructions only for answers that keep a diary. Test project-specific and wildcard `never`.

- **MAJOR — [scripts/app-cli.mjs:165](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:165): Setup failures bypass feedback entirely.** `bringPlatform()` can refuse a first native build for missing SDKs or an unsupported machine, returning at line 159 before capture, recording or nudging starts. These failures produce no feedback prompt, and a subsequent note can describe an older command. **Fix:** initialize command tracking before platform preparation and route these failures through the common completion handler. Cover a missing-SDK refusal.

- **MAJOR — [scripts/feedback-note.mjs:102](/Users/admin/x2scratch/fb1-rv-astra/scripts/feedback-note.mjs:102): Repeated sends overwrite earlier notes’ local archive.** With `always`, the first note moves to `sent/<date>-notes.md`. The next note recreates the same pending filename; `feedback.mjs:160` then renames it over the first archived file. Remote submissions survive, but earlier local entries disappear. **Fix:** use unique archive filenames or safely append acknowledged entries. Test two same-day notes through the real `send()` implementation with mocked HTTP; the current fake sender misses this.

- **MINOR — [scripts/app-cli.mjs:176](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:176): Native automatic builds bypass stderr capture.** Relay activates only for the string `'inherit'`, whereas `test ios`, `agent macos` and other native prebuilds use `[0, 2, 2]` at line 274. Their errors go directly to the outer stderr descriptor, leaving the note’s error tail empty. **Fix:** relay these descriptor mappings while preserving their stdout-to-stderr routing. Add a native prebuild-failure integration case.

- **MINOR — [scripts/feedback-note.mjs:114](/Users/admin/x2scratch/fb1-rv-astra/scripts/feedback-note.mjs:114): Disclosure can be permanently suppressed without being printed.** `disclosure()` writes its marker immediately, but `note()` returns the disclosure text only after awaiting `send()`. If sending fails, the CLI prints only the exception; subsequent attempts suppress the disclosure. **Fix:** emit disclosure before sending and mark it after emission. Test a rejected send on first use.

The permitted test command reported 2 passes; 10 tests failed during temporary-directory creation with sandbox `EPERM`, before their assertions ran.

VERDICT: CHANGES
