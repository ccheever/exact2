# Code review: LLP 1122 T5 step 1, the selection join's template and style rungs, round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `922498a9a9`.
- **Method:** one brief (sha256 `7465de06fcd80ec7e1b8db57a0140ca408227012f8e67bbde045f766f6fdd7d0`), shared by both reviewers, blind to the other review; commits bc485dd65f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 correctness).
- **Disposition:** taken in `e237f00e45`: a held row's unread param is read from the host's own report before its names were scrubbed, so it still refuses; a positional name after a plain value (not an operator word) keeps the template rung (and a param row's template address); held `instances` tested with the press's own copies differing from the label's; the doc's `:active` sentence names the template rung for a row written under the node. Also (review2's pre-read, `6b10281723`): a style is the nearest `style` line above its row only through indented lines, blank ones and comments. Not taken: `opacity=0.5("hello")` with no space (the parser's own positional rule needs none, but no compiled source writes that) — recorded.

**Request changes. Two correctness issues remain.**

1. **Held rows still bypass the unread-param refusal.** At [tamber-inspect.mjs:389](/tmp/rv-dt-t5r3/scripts/tamber-inspect.mjs:389), `named()` receives an already-scrubbed token: constructing `base.held` removed unknown param names. The second call therefore returns `unread: false`.

   Reproduced with inline `:active background-color="param.fill"` while param metadata is unavailable: the ordinary row refuses editing, but the same held row offers `rung: "template"` with `text: "param.fill"`. Round 2’s finding is **not fixed**. Preserve `unread` from the original held report, as the ordinary-row path does.

2. **Positional-argument parity is still incomplete.** The boundary check at [tamber-inspect.mjs:300](/tmp/rv-dt-t5r3/scripts/tamber-inspect.mjs:300) still rejects:
   ```contract
   component App
     state label: string = "Hi"
     view
       text color="red" label
   ```
   Contract parses `label` as a separate positional expression ([parser.rs:1216](/tmp/rv-dt-t5r3/contract/syntax/src/parser.rs:1216)); the colour remains a plain string that `set-row` accepts. The join instead reports it as an expression. With `color="param.ink" label`, the param rung survives but loses its template address, disabling detach. Both results reproduced in memory. Round 2’s Grok review already identified this form; the disposition gives no reason for excluding it.

The other taken fixes check out:

- Held hover/focus rows no longer offer the base attribute’s template address.
- Held `instances` uses the owner’s call-site chains and deduplicates generated labels.
- Sequential escape decoding handles the reported cases and matches the lexer’s Unicode-scalar restrictions.
- Parenthesized and template positional arguments preserve the template address.
- The fixture wire names, held-inline editability assertion, and both protocol-document corrections are fixed.

Leaving `#12`, `1.`, and `.5` permissive in this reader is reasoned: they cannot occur in a successfully compiled matching map. The earlier own-sources startup deferral remains documented and writer-guarded.

One smaller coverage gap: [tamber-inspect.test.mjs:105](/tmp/rv-dt-t5r3/scripts/tamber-inspect.test.mjs:105) gives both owner and selected descendant one chain. Its new `instances: 1` assertion also passes the old implementation. Different-count probes passed, but that distinction needs a regression assertion.

Validation: three existing filesystem-free tests passed, with 51 assertions; additional in-memory checks verified the fixes and reproduced both findings. No Rust builds or file-writing tests were run. The checkout is unchanged.
