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
- **A `key` handler on an input also reaches the `key` handler of an ancestor
  on the web.** `column key=navKey` around `input key=searchKey`: on the web a
  keydown bubbles, so every key in the search field ran both actions (the
  cursor moved twice; Enter opened the session twice); on macOS the field
  editor keeps the arrows, so only the input's handler ran. The web is the
  standard, so the app tracks the field's focus (`focus=`/`blur=`) and the
  ancestor ignores keys while it is focused. A dispatch rule (the innermost
  `key` handler wins, or a contract-level way to stop the bubble) would make
  the two hosts agree without app state.
- **An input's placeholder color cannot be set.** GPUI paints placeholders in
  the theme's `muted`; exact2 has no `placeholder-color` (macOS mutes the
  text color to 30%, the web uses the browser's default, which all but
  vanishes on a dark card). The app layers a `text` under each empty input
  instead. A `placeholder-color` row (CSS `::placeholder { color }`) would
  remove that.
- **The dev loop does not watch `use`d files.** `bun host/web/dev.mjs` rebuilds
  when `app.contract` changes, not when a file it imports with `use` does
  (and a `touch` of `app.contract` alone did not trigger a rebuild either);
  edits to `overlays.contract` only showed after a restart.
- **Lists have no membership test and strings no split.** `includes` takes
  two strings only, so flags travel as a space-joined string; a QR code's
  rows arrive pre-split as records because `split` does not exist.
- **No programmatic scroll to a node.** GPUI's transcript scrolls to an entry
  (Home, End, a search hit's turn) with `scroll_to_reveal_item`. Contract has
  an authored `scrollTop`, absolute, and a `scroll` event; the app reports the
  offset to the model without a repaint and answers j/k/Home/End with a new
  `scrollTop`, but revealing entry N needs its frame, which only an action can
  read. A `scrollIntoView`-style effect (or `scrollTo=nodeId`) would close it.
- **Jobs queued outside `dispatch` wait for the next one.** The `io` resource
  is keyed by the counter `dispatch` returns, so a follow-up job queued while
  applying a reply or a feed event (the PATH plan after a provider update, a
  re-read after a send) runs on the next tick, up to a second later. An
  answer that can say "ask me again" (or a counter the `io` parse can move)
  would remove the lag.
- **One window per app outside document mode.** Ocho's New Window opens a
  second workspace window; ExactMac opens more windows only for apps that
  declare documents with `navigate-new`. The command says it is unavailable.
- **No inline math typesetting.** Upstream renders LaTeX with RaTeX; the app
  shows the TeX source in italic serif. A module-drawn formula would need a
  native inline element inside a `text` run, which Contract has no slot for.
