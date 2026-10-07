# Exact Contract for VS Code

Syntax highlighting and editing defaults for the current `.contract` language.
This extension has no runtime code or dependencies.

To package and install from the repository root:

```sh
cd editors/vscode
bunx --bun @vscode/vsce package --allow-missing-repository --skip-license --out /tmp/exact-contract.vsix
code --install-extension /tmp/exact-contract.vsix --force
```

Run **Developer: Reload Window** in VS Code after installing. Repackage and
reinstall after changing the grammar. No generated package belongs in the repo.

The hand-maintained TextMate grammar is `syntaxes/exact-contract.tmLanguage.json`.
Its language sources are `contract/syntax/src/lexer.rs` and `parser.rs`; built-in
tags, events and host commands come from `contract/lower/src/tags.rs`,
`contract/analyze/src/lib.rs` (`HANDLERS`) and `contract/syntax/src/lib.rs`
(`HOST_COMMANDS`), all three listed by `contract vocab --json`; `cargo test -p
contract --test it vscode_grammar` compares the grammar's lists with them and
names any word missing or no longer the compiler's. See LLP 1006 §2 and LLP
1017.000 P1/P9 for the language and CSS names. Attribute names match structurally, including hyphens,
so adding a CSS property needs no grammar update. The compiler validates names
and values; highlighting is not validation.

Includes declarations (with `timeline`, `sound` and `color-profile`), routes,
resources (with `with`) and mutations, actions with `let` locals, host commands,
record constructors, component `provide` sections, styles, keyframes,
HTML/SVG/native tags, events, types, authored tests, double-quoted
strings, and backtick templates with nested interpolations. Single quotes are
not Contract strings. There is no language server or semantic completion.
