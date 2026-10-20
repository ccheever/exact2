# Code review: LLP 1122 T5 step 3, promote and detach, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `61bc295299`.
- **Method:** one brief (sha256 `162673cb26fb06a336307b39e18d5ca97fb54fd1ffe656269ca4d70710c2381f`), shared by both reviewers, blind to the other review; commits 06507bfb1a. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 5 medium; smaller points).
- **Disposition:** taken in `54d7784b9f`: a new param's or token's kind is the row's (a colour row's `color`, opacity's `number`, a shadow's, `family`, durations and easings, else `length`, a bare number given `px`), the param added in the template's file by its declared component name; a detach writes the param's value (through a default that is exactly one other param, that one's; refused where the design sets it or the default is an expression over another param; a hex or `light-dark(…)` default no longer mistaken for one), is checked against the params of the revision it proposes on (paramsOf), keeps the declaration where the server says another row still reads it (`tamber-param-unknown`), removes in the param's own file only the params of its own component and file that the server says it left unread; a refusal's `hint` is shown; the detach's undo words name where it was written; tests: a shared-reader detach. Not taken: a token default copied as the token's name (the row then reads the token, which is the value the param had; materializing a resolved literal would sever the token); the closure listed before Apply as the server's diff shows it (the stub's diff is a canned first op).

**Changes requested: blockers remain.** I reviewed `06507bfb1a` with the fixes in `82d98133ff` applied. References below use this checkout’s line numbers.

1. **[P1] Detach can use an old default with fresh digests.** [app.ts:220](/tmp/rv-dt-t5r2/apps/design-tool/app.ts:220), [app.ts:831](/tmp/rv-dt-t5r2/apps/design-tool/app.ts:831). `moveOps` reads cached `appParams`, but `paramsOf` checks its revision only for `set-param`. Reproduced through the actual client functions: load `Card.fade = "0.25"`, open `brand.tamber`, change the component default to `"0.75"`, then select the fresh copy. Detach proposes `set-row value: 0.25` and `remove-param fade`, carrying the **new** source digest. The later files-moved check passes. Bind the params used to construct the move to the proposal’s revision, including while another design file is open.

2. **[P2] The default handling rejects valid detaches and prevents D6’s closure case.** [moves.ts:20](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:20). `default.includes('#')` rejects ordinary `"#ffffff"` and `"light-dark(#fff, #000)"`, incorrectly claiming another param is read. Separately, `describe` really does expand dependencies to full names: `b = "param.a"` becomes `card.contract#Card.a` ([tamber_params.rs:284](/tmp/rv-dt-t5r2/contract/cli/src/tamber_params.rs:284)). Thus `row → b → a` is refused **before** the closure loop runs, although D6 requires the literal and removal of the newly unreachable closure. A token default such as `accent.fill` is copied as a token reference, not materialized as D6’s literal. These cases need explicit default/dependency handling rather than a hash-character test.

3. **[P2] Promotion guesses the kind from text instead of the row.** [moves.ts:30](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:30). Actual generated operations include:
   - `background-color="red"` → `kind: "length"`;
   - `opacity=1e-2` → `kind: "length"`;
   - `border-radius=8` → `kind: "number"`;
   - `font-weight="bold"` → `kind: "length"`.

   Both promotion paths use this inference. Derive the kind from the authored row, normalize numeric length defaults where necessary, and refuse unsupported values explicitly.

4. **[P2] Detach always removes a param, including one still read elsewhere.** [moves.ts:21](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:21). With two rows reading `param.fade`, default `"0.5"`, detaching one sends `set-row value: 0.5` **plus** `remove-param fade`. The other reader produces `tamber-param-unknown`; the loop handles only unused params, so this valid detach is unavailable. Remove only params that become unreachable; otherwise retain the declaration.

5. **[P2] Component operations discard their declaring file.** [moves.ts:17](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:17), [moves.ts:21](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:21). If `a.contract` and `b.contract` both declare `Card`, detaching `a.contract#Card.fade` sends an unqualified `remove-param component: "Card"`. The server correctly refuses the ambiguity despite the client knowing the file. Promotion also takes the map’s component name, which can be compiler-renamed for duplicate declarations. Carry the declaring file and authored component name into these operations.

6. **[P2] `unread` accepts diagnostics belonging to another component.** [moves.ts:27](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:27). For a detach from `Card`, a real-format diagnostic naming `other.contract#OtherCard.fade` returns `["fade", "OtherCard"]`. The next proposal attempts removals from **Card**, masking the original error with unrelated removals. Match the complete component identity and extract only the diagnosed param. A component name merely appearing in the diagnostic’s file path also triggers this.

Smaller points:

- The [new test](/tmp/rv-dt-t5r2/apps/design-tool/app.test.contract:1458) checks undo counts, but neither invokes Undo nor asserts the submitted defaults, kinds, or closure diff. The [stub’s diff](/tmp/rv-dt-t5r2/apps/design-tool/stub/tamber-stub.ts:214) contains only `ops[0]`, so it cannot verify that removals are shown before Apply.
- [refusal](/tmp/rv-dt-t5r2/apps/design-tool/app.ts:45) displays diagnostics but drops the server’s `hint`.
- Detach’s [undo label](/tmp/rv-dt-t5r2/apps/design-tool/moves.ts:39) omits the destination row/location, making different detach destinations indistinguishable.

The transaction wiring is sound: successful moves retain the reviewed operations, display the server’s final diff before Apply, and create one transaction and one undo entry. Numeric detach defaults become JSON numbers; promotion defaults remain strings as required by the protocol. Clashing names are submitted unchanged for refusal. The closure loop terminates after at most five proposals.

Validation used read-only source inspection and in-memory Bun probes of the actual client functions, with mocked server replies. No builds or file-writing tests ran; the checkout remains unchanged.
