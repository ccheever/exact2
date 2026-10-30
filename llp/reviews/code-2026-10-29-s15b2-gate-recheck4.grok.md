# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck4, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `77ba409ed14057f0553d9656ed1182a0ee17e0970ed235e1987eaaee6569bb6c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 medium, 1 low).
- **Disposition:** as Astra's.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g5-brief.txt. At g5, Grok was CLEAN. Astra's P2: a binding declaring its own `slot` and rendering `children` beside its PersonRow places its caller's nodes, whose chains and walks never name the binding, so both map checks pass.
This commit adds declaresSlot(text, component). It finds the declaration by `^component\s+<name>` followed by whitespace, end of line or a comment, so `B-old` isn't `B`. The declaration ends at the next top-level declaration keyword, not at a column-0 `)`. Any line that is the word `slot` alone (an optional comment allowed) is the slot declaration. consumerPixels fails a binding that declares one, or that isn't declared in its named file. S15B2_BINDINGS names the files.
Check this closes the forwarding hole: a component's children can only come from its own slot. Check the declaration finder against legal Contract (keywords, comments, a column-0 `)` inside a call), and whether `slot` can appear alone on a line inside a declaration in any other meaning. Is anything else left under the node-level scope?
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD on the s15b2 gate branch, read-only, starting from the earlier briefs and `git show HEAD`.The g5 check only looked at the source map. This commit adds a text scan for a binding's own `slot`. I'll read that scan against the Contract grammar and the map checks.The forwarding hole is still open. `declaresSlot` does not use Contract's declaration boundary, so a binding can still declare `slot` and place `children` beside its PersonRow.

**P1** `scripts/stage12-named.mjs:202` — The end test treats a column-0 line starting with `use`, `enum`, `derive`, `action`, `state`, or `const` as the next declaration. Inside `()`, `[]`, or `{}` the lexer keeps that line in the component. It force-closes an open bracket only for `component`, `font`, `shape`, `style`, `decoration`, `atom`, `fn`, `test`, `always`, `after`, and `never` (`contract/syntax/src/lexer.rs:130-147`). Those six words are legal there: `state` is a legal parameter (`ident` refuses only the sixteen reserved words, `contract/syntax/src/parser/names.rs:20-23`), and a named argument may be any word. This compiles and is reported as declaring no slot:

```contract
fn label(state: string): string = state

component PersonResultRow
  props
    person: Person
  derive title = label(
state=person.name
)
  slot
  view
    PersonRow(id=person.id, name=title)
    children
```

The `slot` line is past the slice, so `declaresSlot` returns false. `children` still places the caller's nodes beside PersonRow. Their chains do not contain the binding, and their walk's first step is not that PersonRow call, so `bindingsCheck` and `bindingShape` both pass. `consumerPixels` then accepts the binding (`scripts/stage12-gate.mjs:488`).

The name boundary holds: `B-old` does not match `B`, and a trailing `//` on the component line matches (`:200`). A column-0 `)` does not end the window. A trailing `//` on the slot line matches the section (`:205`). The new test (`scripts/stage12-named.test.mjs:130-132`) covers those three and never puts one of these six words in front of `slot`.

**P2** `scripts/stage12-named.mjs:205` — A line that is only the word `slot` is also a name. `slot` is contextual, so it is a legal binder and a legal expression (`contract/syntax/src/parser/names.rs:7-10`). Two compiling forms match `^\s+slot\s*(//.*)?$` with no slot section. A `provide` entry may be the bare name (`contract/syntax/src/parser/component.rs:125-130`); the corpus writes that as its own line (`contract/corpus/provide.contract:8-9`). A parenthesized expression may be that name on its own line (`derive label = (` / `slot` / `)`). `declaresSlot` returns true, and the gate rejects a binding that declares no slot. A view line that is only `slot` does not compile (it is an unknown tag), and a bare name in an action is `syntax-expected-statement`, so those two are not this bug. Strings and templates are single-line, so a string cannot hide or fake the section line.

**P3** `scripts/stage12-named.mjs:202` — That same end regex omits real top-level declarations: `members`, `font`, `sound`, `routes`, `decoration`, `atom`, `keyframes`, `timeline`, `color-profile`, `theme`, `always`, `after`, and `never` (`contract/syntax/src/parser.rs:502-575`). The scan reads into what follows. A later `members` body may contain P2's name line (`state` and `derive` are allowed there). The binding is flagged for a line in the next declaration. This direction cannot hide the binding's own `slot`, because reading further only adds lines.

Caller nodes enter a component only as its slot fill. Nodes indented under a use of a component with no slot are `syntax-no-slot` (`contract/syntax/src/inline/use_node.rs:339-345`). `children` with no slot is `syntax-children-without-slot` (`contract/syntax/src/inline.rs:509-512`). There is no render prop. An exact window would close the beside-row hole, and the map checks would still cover every node the binding itself draws. Control flow stays outside the node-level terms. Nothing else in that scope remains.

VERDICT: CHANGES
