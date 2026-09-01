# Boot check misses valid script syntax

**Status:** Closed
**Resolution:** The boot check now parses HTML source attributes and JavaScript module syntax instead of relying on narrow regexes.
**Systems:** Tooling, Web host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** rules/RULES.md boot budget

The fifth check finds page modules with
`/<script[^>]*src="([^"]+)"/` (`scripts/boot.mjs:29-44`). Valid HTML using a
single-quoted or unquoted `src` is invisible to that expression. The inline
script expression still sees `src=` and therefore does not reject it either.
For example, `<script type="module" src='./app.js'></script>` produces zero
queued modules and zero inline scripts, so an app module can pass the hard
no-app-JS budget.

The static module scan also follows `import` statements but not re-exports
such as `export * from './app.js'`.

Parse HTML attributes and ECMAScript module specifiers with parsers (or a
small deliberately complete tokenizer), fail closed on unsupported syntax,
and add bypass fixtures for single quotes, unquoted attributes, re-exports,
comments, and malformed tags.
