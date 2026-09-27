# Strings: MessageFormat 2 syntax (only `{name}` built, the rest refused), `lang`/`dir` from the resolved table, and the locale on `exactTime`

**Status:** Open
**Systems:** Contract strings (`contract/cli/src/strings.rs`, `contract_types::strings`), plan strings, web/Apple/Linux hosts, runner time facts
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1060 (the 2026-09-27 ruling)

**Do:**
- Parse table values as MF2 simple messages. Implement `{name}` (and `{$name}` if MF2's variable spelling is adopted; say which in LLP 1060). Refuse at compile time, by name, `.match`/`.input`/`.local`, functions (`{$n :number}`), markup and any other construct, never passing them through as literal text. Keep MF2's escaping (`\{`).
- Set `lang` and `dir` from the resolved table: on the web, `document.documentElement.lang`/`dir` (and in the render server's document); on Apple, the accessibility language and the semantic content attribute; on Linux, whatever the painter's shaping and accessibility read. Right to left comes from the locale's script (CLDR likely-subtags for `ar`, `he`, `fa`, `ur`, …).
- Put the resolved table's locale on `exactTime` next to `locale`, so TypeScript sources stop copying the RFC 4647 lookup. Update the apps that copied it.
- Tests: an MF2 plural in a table is refused with its name; switching to `ar` sets `dir=rtl` on each host.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
