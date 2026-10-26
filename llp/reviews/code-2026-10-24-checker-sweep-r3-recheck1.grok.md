# Code review: the checker sweep (after the checker bound), round 3, narrow re-check 1, 2026-10-24 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `5fffcf8baf`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed before the rebase onto main.

Clean.

The three commits after `03aba06dd3` close the round-3 findings. Nothing new in that range is a bypass, a super-linear path, or an echo.

- **Ops cap.** `overOps` in `scripts/tamber-ops-limit.mjs` refuses any client `ops` array longer than 1024, with no `reholds` / `target` exception. The server door calls that form. Accept and revert still apply a held import on the server. The 1.17 paragraph, the re-hold tests, and the QUEUE entry are gone. On main, `edit-held` sends `proposal`, `index`, `value`, and `items` only, and it is what a 1.17 design tool calls.
- **`mixed`.** For a segment of `set-token`, `set-dial`, and `add-token` with no `before`, it appends the new tokens with one parse, then applies the values with one, and only when both kinds have at least two operations and no set names a new token by name or role. A value only rewrites its own token or dial lines, which sit above where a new token is inserted (after the last token's lines, including an indented closing comment). A when-line insert stays inside that token either way, so the following block, a missing final newline, and dial lines come out the same text. Any refusal returns `None` and `run` steps. The property test covers dials, when lines, a closing comment, and no final newline; the 6,000-op alternating case is two parses and undoes.
- **DTCG.** Set rank is the first `tokenSetOrder` hit, or the last path segment when exactly one set ends with it. Theme groups keep first-seen order via a map. Both match the old scans.
- **Pins.** `marked` indexes each `set-dial` and `set-token` once. Name and `when` match by the same JSON equality as before, in file order.
- **Padded value.** The refusal quotes `v.trim()`. `over_limit` still measures that trim (4096 bytes, and its words do not quote the text). `Diagnostic::new` still cuts the message at 1024 bytes.
- **Batch fallback.** A client request is at most 1024 operations. A server-held import is applied on the design it was planned on, digest-guarded, so that list does not take the one-op walk. `edit-held` only substitutes a token value; it does not reopen the old re-hold of the whole list.
