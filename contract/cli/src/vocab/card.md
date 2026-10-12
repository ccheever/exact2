<!--
`contract vocab` with no name prints the card (LLP 1116.010 D1): what a builder looks up one call at a
time, in one read, at most 8 KB (`contract/cli/tests/it/vocab.rs` holds it). `card.rs` generates it from
the tables: the functions from the roster and the grammar, the parts from `exact:content`'s calls, the
steps from docs/recipes/tests.md, the agent's operations from scripts/help.mjs, the symbols from the
schema. This file holds what no table says, and a test checks it against them: every tag, attribute and
event named below exists, every function it rewrites exists, every `contract` block compiles.

A section is `## name`. `functions`: `name: text` replaces that function's generated line, `name: (text)`
adds words to it. `tags`: `tag tag | attribute attribute | event event`, checked word by word.
-->

## intro
Exact card: what an app writes. `contract vocab <name> …` answers a name left out (a recipe, a rule);
`--all` lists every tag, attribute and function.

## functions
at: at(list<T>, i) -> option<T>  (none past the end; -1 is the last)
calendarDiff: (whole years or months between two YYYY-MM-DD dates)
concat: concat(list<T>, list<T>) -> list<T>
filter: filter(list<T>, x => bool) -> list<T>
first: first(list<T>) -> option<T>
formatDate: formatDate(ms, offsetMinutes, "medium"|"month-year"|"iso") -> string
formatDecimal: (cents to "12.34"; digits a literal)
formatNumber: formatNumber(n, "compact"|"decimal"|"percent"|"currency", "USD") -> string  (1,234.5; 26%)
formatTime: ("2:13 PM"; minutes east of UTC)
frame: (in an action)
includes: includes(text|list, x) -> bool
indexOf: indexOf(text|list, x) -> number  (-1 for none)
isEmpty: isEmpty(text|list) -> bool
join: join(list<string|number|bool>, sep) -> string
length: length(text|list) -> number
map: map(list<T>, x => U) -> list<U>
measure: (in an action)
parseNumber: (none for "", "12px", "1.2.3")
performanceNow: (ms since launch)
round: (half up)
slice: slice(text|list, start, end?) -> text|list
t: t("key", name=value) -> string  (the locale's table)
toFixed: (digits a literal, 0 to 100)
toString: toString(x) -> string

## tags
view box row column main nav article aside section header footer hr | id testId role title tabindex | press hover key keyup focus blur contextmenu dblclick
text | markup | selectionchange
image | alt | load
button | disabled buttonStyle destructive | press
link | href | press
input | value type inputmode min max step maxlength placeholder enterkeyhint autocomplete autocapitalize autofocus disabled readonly checked name accept multiple | input change submit
textarea | value placeholder maxlength enterkeyhint | input change select
select | value disabled | change
option | value
progress | value max
list | listStyle appearance virtualized refreshing scroll-start | reachstart reachend refresh
scroll | scrollTop navigationScroll keyboardDismissMode | scroll
dialog | closedby | cancel

## rules
- An event hands its action its payload after the captured arguments: the text for a field, a `select` or a date; the bool for a checkbox or switch; the number for a range; nothing for `press` and `submit`. `change=set(i.id)` calls `set(id, payload)`.

```contract
component App
  state name = ""
  state on = false
  action setName(v: string)
    name = v
  action setOn(b: bool)
    on = b
  view
    column
      input value=name input=setName inputmode="decimal" min=-40
      input type="checkbox" checked=on change=setOn
      select value=name change=setName
        option "A" value="a"
```
- `select value=v change=set` holds `option "Label" value="x"`. Tabs: a `row role="tablist"` of `button role="tab" aria-controls="panel-x" aria-selected=… press=…`, each panel a `column role="tabpanel" id="panel-x"`; `routes nav`, `tab`, `select(nav, name)`.
- `state x = 0 persist` on the root keeps a number, string or bool (or an option or list of one) up to 16 KB across launches; records belong in a source.
- A number field's `min`, `max`, `step` take numbers, negative bare (`min=-40`); with `inputmode="decimal"` a negative `min` gives the pad its ± key.
- A symbol is `"symbol:<role>"`, never a made-up name; `symbol:sf/<name>` passes an SF Symbols name to Apple.
- Not in Contract, each with what to write in `contract vocab <name>`: abs pow sqrt sort reduce find some every contains toUpperCase padStart parseInt.
- `a - b` subtracts with spaces; a unit is quoted (`width="50%"`); `when` and `else` choose in a view.
