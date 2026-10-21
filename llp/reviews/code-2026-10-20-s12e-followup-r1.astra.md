# Code review: Stage 12 E follow-up: PostButton's pressed and dim params, the packages' sizes and type as tokens, neutral colours as roles (LLP 1139 §5 E), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `33083088ec`.
- **Method:** one brief (sha256 `20e8865fdcd985444d18075fe7e874d698615806f37ad5e0ebcacda7a093d166`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocker; one should-fix (radius.full squares the terminal's avatar ring), three nits
- **Disposition:** all taken: cards.avatar-corner = 999px; avatar-ring says why it is no role; exact RGBA and the pressed param asserted; Dir built first

No BLOCKER found.

- **SHOULD-FIX — [packages/cards/common.contract:46](/tmp/iosfix-rv-fu1/packages/cards/common.contract:46):** `radius.full` changes the terminal avatar border. Its terminal value is **0** (`kernel/tables/schema.json:6001`), and the terminal painter switches from rounded `╭╮╯╰` corners to square `┌┐┘└`. The graphical hosts clamp it correctly for these square avatars. Preserve the terminal appearance with a package radius token, or explicitly accept this additional visual change.

- **NIT — [packages/cards/cards.tamber:21](/tmp/iosfix-rv-fu1/packages/cards/cards.tamber:21):** `cards.avatar-ring` still uses the old neutral hex palette. It does not follow the platform separator role, contrary to the new header’s description of neutral defaults. Use a role-backed value or document this exception.

- **NIT — [contract/cli/tests/it/library_packages.rs:87](/tmp/iosfix-rv-fu1/contract/cli/tests/it/library_packages.rs:87):** The inequalities catch the original disabled-fill regression, but swapping `fill` and `dim` would still pass. Assert the exact configured red and green values. The new `pressed` override also remains untested.

- **NIT — [contract/cli/tests/it/library_packages.rs:43](/tmp/iosfix-rv-fu1/contract/cli/tests/it/library_packages.rs:43):** Construct `Dir` before filesystem setup. Currently, a setup `unwrap()` panic leaves the partial directory behind. Cleanup after the helper returns—including compilation or assertion failures—is correct.

Otherwise, the requested literals are covered. Bound SVG/image dimensions and the mixed `"0px"`/token branch have valid resolution paths. Hairline and role defaults are admitted by the library rules, and the changes preserve the gate’s exports and variant coverage.

Source review only; no files modified or build/test commands run.
