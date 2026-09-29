# Contract frictions met authoring Ocho

**Status:** Open
**Systems:** Contract (types, lower), kernel (box-shadow)
**Severity:** P3
**Author:** Claude Fable 5.1, building Ocho for Eliot Hertenstein
**Date:** 2026-09-28
**Related:** LLP 1017.000, LLP 1017.003, LLP 1035.005

Each of these stopped a compile once while writing `apps/ocho/app.contract`;
each has a workaround, and each workaround is less clear than the thing it
replaces.

- **No empty list literal.** `match m { case some(x) => x.items, case none => [] }`
  refuses `[]` (`syntax-expected-expression`). The workaround is
  `filter(someList, c => false)`, which reads as a trick. An `[]` whose type
  comes from the other branch (or a declared `list<T>`) would do.
- **`class=` takes one name or a two-way choice only.** A three-state row
  (selected / hovered / plain) cannot be `class=(a ? A : (b ? B : C))`
  (`type-class-name`); the workaround is `class=Plain` plus an overriding
  `background-color=(…)` with the nested ternary. Allowing any expression
  whose branches are all style names would keep the styles where they belong.
- **String search is not under the web's name.** The roster has `contains(s,
  t)` for strings, which the author did not find while looking for the web's
  `includes`; neither `startsWith` nor `endsWith` exist, so highlighting the
  launcher rows whose two-digit code starts with the digit typed needed a
  `group` field computed by the module. PR #59 renames `contains` to
  `includes` and adds the other two.
- **`box-shadow` takes one shadow.** The common "soft shadow + hairline ring"
  idiom (`0 24px 64px …, 0 0 0 1px …`) is refused; the workaround is a border.
  The kernel row exists; the comma list is the missing part (the message says so).
- **Reserved words as parameter names.** `fn f(state: string)` and
  `action k(key: string)` are refused (`syntax-expected-name`); the message is
  clear, but `state` and `key` are natural names for a terminal client.
- **A virtualized list's empty state** must live outside the list (exactly one
  direct `each`), and its bottom padding must be zero, so the "No sessions"
  message and the list are siblings under a `when`. Documented in the refusal;
  noted here as friction only.
- **A module prop named like a style row is silently taken by the row.**
  `ghostty-terminal appearance=…` compiled, but `appearance` is a known
  attribute, so the module never saw it and the terminal never changed theme;
  renaming the prop to `scheme` fixed it. A native module tag's leftover
  attributes should either be all its own, or a known-row collision on a
  hyphenated tag should be a refusal with a message.
