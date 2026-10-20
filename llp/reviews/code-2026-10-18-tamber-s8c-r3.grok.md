# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `8e0029eee..cfcef760e` (C1 round 3, C2 round 2).
- **Method:** one brief (sha256 `c9df51f7861670e417900b21a8f9770ba6070d614062091bc29ac7af97499861`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 182f4d61c (C1, with theme2's 08:42 ruling) and 9035e2cf9 (C2), folded.

---

## BLOCKING

**The terminal is still judged.** `contract/lower/src/control_roles.rs:98-103`. `cfcef760e` says tvOS, watchOS, and the terminal are not judged. The `continue` only fires when `target.name` is absent from `CONTROL_HOSTS` (`kernel/build/codec.rs:761`), and `terminal` is in that list. tvOS and watchOS are skipped; a shipped `terminal` key (`app.json` `host` / `deploy.store` / `deploy.binaries`) is not. Every terminal cell is `none`, so each drawn control and set parameter warns, and `warnings.rs:194` keeps only 20. `docs/contract-for-agents.md:3129` still says every shipped target.

## SHOULD-FIX

**A class row the node overrides still classifies the control.** `contract/lower/src/control_roles.rs:163-164`, `:195`, `:229`. `rows()` keeps every class attribute and then the node's, and `control()` uses `.any()`. `class.rs:40-44` drops a class row the node sets, and `buttons.rs:69` takes the later literal.

- `style Bare appearance="none"` plus `input class=Bare appearance="auto"` returns `None` here, so the native field's `none` cells are missed.
- `style Primary buttonStyle="bordered-prominent"` plus `button class=Primary buttonStyle="bordered" disabled` is `button-prominent:disabled`, a false warning on the iOS, web, Linux, and Windows `none` cells.

`contract/cli/tests/it/control_roles.rs:119` only covers a class the node does not override, and `appearance="none"` on the node itself. A conditional class the node does not override is the intended "either way" rule.

## NIT

- `kernel/build/validate.rs:262-268` accepts `exact: some why`. `Some(why)` only checks that the why is non-empty, so the "exact needs no why" comment is not what the assert does.
- `llp/1121-tamber.rfc.md:3519` omits `; LLP 1121 D9`. The emitted message (`control_roles.rs:132-138`) and `docs/contract-for-agents.md:3131` include it.

## Round-1 / round-2 fixes that did land

C1 (`8e0029eee`): macOS `checkbox:disabled` and `button-prominent:disabled` are best-effort (`schema.json:5917`, `:5924`); frames that disagree by more than 0.5 pt are `unmeasured` and fail the run (`scripts/measure-controls.mjs:216`, `:222`); `SCHEMA_DIGEST` is `0x24a0_0cfe_5916_60dc` (`kernel/src/wire/codec.rs:732`).

C2 (`cfcef760e`), besides the terminal skip: the cited accent is the node's own row (`control_roles.rs:65-68`); a field's own row exempts only when it is always present (`:71-74`); `appearance="none"` on the node, a computed `type`, and type case-folding behave as the commit says; `unlowered()` runs at the start of lowering.
