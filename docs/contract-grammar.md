# Contract grammar and vocabulary reference

This is a descriptive reference for the compiler on `main` on 2026-10-02, not a
proposal for new syntax. The [human guide](contract-for-humans.md) teaches the
language; the [agent guide](contract-for-agents.md) gives the implementation loop.
The parser, type checker, and lowering rules are authoritative when this summary
and implementation disagree.

The grammar below describes authored forms. Context-sensitive restrictions are
listed separately: parsing a construct does not imply it types, lowers, bakes,
or runs on every host. `IDENT`, `FIELD`, and `TAG` are identifier tokens accepted
in their respective contexts; reserved-word handling differs by context.

## Contents

- [Lexical rules](#lexical-rules)
- [Declarations](#declarations)
- [Components and actions](#components-and-actions)
- [Views](#views)
- [Expressions and types](#expressions-and-types)
- [Authored tests](#authored-tests)
- [Standard functions and intrinsics](#standard-functions-and-intrinsics)
- [Built-in tags](#built-in-tags)
- [Events](#events)
- [Host commands](#host-commands)
- [Semantic restrictions](#semantic-restrictions)

## Lexical rules

The notation uses `=` for a production, `|` for alternatives, `[ … ]` for optional
syntax, `{ … }` for repetition, and quoted text for literal tokens. `NL`, `INDENT`,
and `DEDENT` describe logical line boundaries. `block(X)` means an indented run of
`X` entries, `NL [ INDENT { X } DEDENT ]`; blank and comment-only lines do not
count as entries. Most bodies may be empty (a shape, a style, `props`, an action,
an `each` body, a `case` arm, a test). The bodies of `if`/`else` and `when`/`else`,
a `keyframes` or `font` block, and a component need at least one entry.

- A name starts with `[A-Za-z_]`, followed by ASCII alphanumerics/underscores or
  a hyphen immediately followed by an ASCII letter. Thus `font-size` is a name,
  `a-b` is a name, and `a - b` is subtraction.
- Number tokens begin with a digit. A dot is part of the number only when a
  digit follows (`12`, `12.5`), so `rows.0.steps` is a list index and then a
  field. Numeric parsing rejects invalid spellings. Negative values are unary
  minus applied to a number. Exponents, separators, leading-dot numbers, and a
  trailing dot are not supported forms. Units belong in quoted strings.
- Strings use `"…"`. Escapes accepted by the ordinary string lexer are `\n`,
  `\t`, `\"`, `\\`, ``\` ``, and `\$`. Single quotes are not delimiters.
- Templates use backticks and `${expression}`. Interpolation scanning balances
  nested braces, strings, and templates. A template is one physical source line.
  Template text is verbatim: a backslash is never an escape and stays in the
  text, and `\${` still interpolates. Use an ordinary string when you need an
  escape.
- A hex color may be written bare, as CSS writes it: `#` and 3, 4, 6 or 8 hex
  digits (`color=#1f9d62`, a keyframe's `background-color=#1f9d6244`) is the
  string `"#1f9d62"`, and the formatter prints it quoted. Any other `#` is
  refused, saying so.
- `//` starts a line comment outside a string/template. No block comments;
  `#` is not a comment.
- Indentation uses spaces; a tab in indentation is refused. At bracket depth zero, indentation emits
  block tokens. Inside `()`, `[]`, and `{}`, newlines and indentation continue
  the logical expression instead.
- No statement semicolons, single-quoted literals, JSX, or arbitrary braces for
  value interpolation. `{}` belongs to inline option-match syntax, not objects.

The lexer reserves no separate keyword token kind. Sixteen words are reserved,
and only where a name is bound ([`names.rs`](../contract/syntax/src/parser/names.rs),
LLP 1088 D5): `when`, `if`, `else`, `each`, `in`, `match`, `case`, `as`, `fn`,
`and`, `or`, `not`, `true`, `false`, `none`, `some`. A binder is a component's
props, injects and provided names; states, derives, resources, mutations and actions; action and
`fn` parameters and `fn` names; `let`, an `each` item and index, `case some(x)`,
an arrow parameter; and a shape name (a shape name builds the shape, so
`shape none` is refused). There a reserved word is refused as "`in` is reserved
in Contract (it shapes an expression); choose another name", and is never a value.

Every other keyword is contextual: a keyword only where its construct starts, a
name at every binder and in every expression. They are `component`, `font`,
`shape`, `style`, `from`, `state`, `derive`, `resource`, `mutation`, `action`,
`task`, `mount`, `view`, `props`, `provide`, `inject`, `slot`, `children`, `key`,
`queue`, `refresh`, `writes`, `test` and `expect`. `refresh`, like `send` and `let`, begins
a statement only before a name, so `action refresh`, `press=refresh` and
`refresh feed` each have one parse. Shape fields, named arguments
(`Flags(none=1)`), members after `.` and attribute names (SVG's `in`) admit every
word.

## Declarations

```ebnf
file          = { declaration } ;
declaration   = use | shape | function | style | keyframes | font
              | routes | component | test ;
use           = "use" use-name { "," use-name } "from" STRING NL ;
use-name      = IDENT [ "as" IDENT ] ;
shape         = "shape" IDENT block(field) ;
field         = FIELD ":" type NL ;
function      = "fn" IDENT "(" [ typed-params ] ")" ":" type "=" expr NL ;
typed-params  = typed-param { "," typed-param } [ "," ] ;
typed-param   = IDENT ":" type ;
style         = "style" IDENT block(style-line) ;
style-line    = { FIELD "=" style-literal } NL ;
style-literal = STRING | NUMBER | "-" NUMBER | "true" | "false" ;
keyframes     = "keyframes" IDENT block(keyframe-line) ;
keyframe-line = selector { "," selector } { FIELD "=" frame-value } NL ;
selector      = "from" | "to" | NUMBER "%" ;
frame-value   = STRING | NUMBER | "-" NUMBER | constant-call ;
constant-call = IDENT "(" [ arguments ] ")" ;
font          = "font" STRING ( "=" STRING NL | block(font-face) ) ;
font-face     = NUMBER [ "italic" ] "=" STRING NL ;
routes        = "routes" IDENT block(route-row) ;
route-row     = [ "tab" ] IDENT STRING { attribute } NL [ route-children ]
              | "notfound" { attribute } NL ;
route-children = INDENT route-row { route-row } DEDENT ;
```

A font face's weight is a whole number in 1–1000. Keyframe percentages are in
0–100; `from` and `to` are 0% and 100%. Lowering checks whether keyframe properties
are animatable and constant-call values can be evaluated at compilation.
Styles accept literal style attributes and explicitly styleable props (currently
`buttonStyle`), not arbitrary expressions or event props.

A `use` specifier is a relative path (`./` or `../`) to a `.contract` file that
stays inside the using file's root — the app directory, or the package it
belongs to (`contract-use-path`); an `exact:` built-in (`exact:motion`;
`contract-use-builtin`); or a package name, `name[/sub]` or
`@scope/name[/sub]`, found in the nearest `node_modules` above the using file
and mapped through its `package.json` `exports` (a string, or the `contract` or
`default` condition), else `index.contract` (`contract-use-package`).
`contract sources <file>` prints every file a compile reads, as JSON. A file's
names are its own declarations and the names its `use` lines list, each
optionally renamed with `as` (LLP 1091): a component, shape, function, style,
keyframes, or timeline, declared by the used file or named by its own `use`
lines. A name another file declares and this one does not name is refused
(`contract-use-missing`): one refusal a file, naming every `use` line the file
lacks — the line it has for that file, extended, or a new one — and `contract
fmt --uses <root.contract>` writes them (`bun exact.mjs update` does too, for an
app outside this repo). A name two files declare, or one the compiler gave on a
collision (`Card__ui`), is left to the author and said. The keyframes an `animation`, `animation-name` or
`exit-animation` literal names, and a `clock(Name)` literal, resolve in the
file that writes them; a name computed at run time is matched as written. Beside
a computed value, a word is renamed when it is the name whatever the value is;
one that is the name for some values and a keyword for others (`${x} linear
1s`), when it is also keyframes another file's of its name renamed, is refused
(`contract-animation-ambiguous`): quote the name (`'linear'`).
Fonts are app-wide. A used file cannot declare routes. The root file's first
component remains the root. A name both declared and used
(`contract-use-shadows`), one name used from two declarations
(`contract-use-duplicate`), and cycles are refused.

A route pattern is an absolute path with literal or whole `:name` segments.
Indentation determines its parent. The optional fallback is `notfound` without
a path. Route policy fields use attribute syntax and are validated by the
[route/document compiler](../contract/lower/src/routes.rs).

## Components and actions

```ebnf
component     = "component" IDENT block(section) ;
section       = "props" block(field)
              | "inject" block(field)
              | "provide" block(provider)
              | "slot" NL
              | ( "state" | "derive" ) IDENT "=" expr NL
              | resource | mutation | action | task
              | "view" block(node) ;
provider      = FIELD [ "=" expr ] NL ;
resource      = "resource" IDENT "=" source-call
                [ "with" expr { "," expr } ] "as" "shape" type
                [ "else" source-call ] NL ;
mutation      = "mutation" IDENT "as" "shape" type [ "queue" ]
                [ "refreshes" IDENT { "," IDENT } ]
                [ "then" IDENT ] NL ;
action        = "action" IDENT [ "(" [ action-params ] ")" ] block(statement) ;
action-params = action-param { "," action-param } [ "," ] ;
action-param  = IDENT [ ":" type ] ;
task          = "task" IDENT ( "mount" | gate ) block(schedule) ;
gate          = "when" expr [ "key" "=" expr ] | "key" "=" expr ;
schedule      = "every" "(" ( expr | "frame" ) "," IDENT ")" NL
              | "after" "(" expr "," IDENT ")" NL ;
statement     = IDENT "=" expr NL
              | "let" IDENT "=" expr NL
              | "send" IDENT "=" source-call NL
              | "refresh" IDENT NL
              | IDENT "(" [ arguments ] ")" NL
              | if
              | statement-match ;
if            = "if" expr block(statement) [ "else" ( if | block(statement) ) ] ;
statement-match = "match" expr block(statement-case) ;
statement-case  = "case" "some" "(" IDENT ")" block(statement)
                | "case" "none" block(statement) ;
source-call   = IDENT "(" [ arguments ] ")" ;
```

There is one `props`, `inject`, `provide`, `slot`, and `view` section at most in
a component. Only the root may own resources, mutations, and tasks. A task's
body has exactly one schedule. A gated task (`when cond`, LLP 1092) has its
timer only while `cond` holds, armed from the commit that made it true; `key=`
re-arms it from the commit that changed the key (compared as an `each` key),
and `key=` alone is `when true key=…`. The gate is a bool (`type-task-gate`), the
key a string, number or bool (`type-task-key`), and neither reads `now()`,
directly or through a derive or a `fn` (`analyze-task-gate-clock`). The named timer action takes no parameters;
a millisecond interval is a literal whole number of at least 1.

`else if c` is `else` around one nested `if c`, and `else when c` in a view
is `else` around one nested `when c`: the same tree, so the same plan. The
keyword after `else` is its construct's own (`syntax-else-keyword`).

Both option-match arms are required exactly once. Actions have no loops, returns,
or awaits. A standalone call statement `name(args)` is a host command when `name`
is one. When `name` is also an action, an `action` prop or an injected
action of the component the statement is written in, the call is refused
(`syntax-call-ambiguous`, LLP 1089 D1), inside an action of that name too:
rename the action (Caltrain's wrapper is `action chooseScheme`, which calls
the command `setScheme(s)`). When `name` is not a host command, it calls an action of the same component, an
`action` prop or an injected action (LLP 1089), never a function. A call is
expanded in place: the callee's statements run where the call stands, in the
caller's one commit, reading the state the action started with. Its arguments
complete the callee's parameters after any curried where it was bound; no event
payload is appended. A call gives no value (`type-call-value`), passes no action
as an argument (`type-call-action-arg`), and never recurses, directly or through
other actions (`syntax-call-cycle`). An action whose calls would expand past
1,024 statements is `syntax-call-size`. `let` is recognized as the
local-declaration statement when followed by a name; a writable slot named `let`
can still appear in an ordinary assignment.

A resource's optional `with` supplies one or more request context expressions,
after its call arguments and before `as shape`. The source receives both lists
in order; a change to either asks again. Only admission of an eligible persisted
answer at boot compares the call arguments alone (LLP 1027.005). An empty call
is valid; an empty `with` is not. `else` keeps its place after the shape.

A resource's `else` is a placeholder. `else empty()` is the resource type's zero,
with `field=constant` overrides for a record. `else source(values…)` names a source
whose arguments are plain values, not state; it is answered once at build.
A mutation's `then` names a parameterless action and follows `refreshes` when
both are present. `queue`, right after the shape, makes every send of the
mutation wait its turn (LLP 1092): one request in flight, later sends asked in
order, each after the reply before it and its `then`. Action effects are inferred: `writes` clauses are refused.
Component `contract` sections are also refused.

## Views

```ebnf
node          = element | component-use | when | each | view-match | children ;
element       = TAG { attribute | expr | special-word } NL [ element-body ] ;
attribute     = FIELD "=" expr | "autofocus" ;
element-body  = INDENT { continuation } { node } DEDENT ;
continuation  = FIELD "=" expr { attribute } NL ;
component-use = CAPITALIZED_IDENT "(" [ named-args ] ")" NL [ node-children ] ;
named-args    = FIELD "=" expr { "," FIELD "=" expr } [ "," ] ;
node-children = INDENT node { node } DEDENT ;
when          = "when" expr block(node) [ "else" ( when | block(node) ) ] ;
each          = "each" IDENT [ "," IDENT ] "in" expr "key" "=" expr block(node) ;
view-match    = "match" expr block(view-case) ;
view-case     = "case" "some" "(" IDENT ")" block(node)
              | "case" "none" block(node) ;
children      = "children" NL ;
special-word  = "document" | "switch" | "multiple" ;
```

`special-word` is contextual: `scroll document`, and the admitted input forms,
not three universal positional flags. The lowering pass restricts an element's
positional values, children, and properties. `button "Label"` inserts a text
child. `autofocus` is the bare boolean convenience spelling.

A deeper element line is treated as an attribute continuation only when it starts
with `name=`; once child nodes begin, later lines are children, not continuations.
Continuations may sit deeper than the children that follow them; `fmt` moves them
to the children's level.
A repeated attribute on one element is refused. A component use takes named
arguments; its name starts uppercase to distinguish it from an element.

An `each` key is a string, number, or boolean, with unique stable values required
for useful row identity. Its optional second binding is the numeric row index.
`when` conditions are boolean and `match` subjects are options. Root regions are
refused: wrap them in an element. `children` requires a declared slot.

## Expressions and types

```ebnf
type          = "number" | "string" | "bool" | "unit" | IDENT | "action"
              | "option" "<" type ">" | "list" "<" type ">" ;
expr          = ternary ;
ternary       = binary [ "?" expr ":" expr ] ;
binary        = unary { binary-op unary } ;
unary         = ( "-" | "not" | "!" ) unary | postfix ;
postfix       = primary { "." FIELD } ;
primary       = NUMBER | STRING | TEMPLATE | "true" | "false" | "none"
              | "some" "(" expr ")" | list | IDENT
              | IDENT "(" [ arguments ] ")"
              | "(" expr ")" | inline-match ;
list          = "[" [ expr { "," expr } [ "," ] ] "]" ;
arguments     = argument { "," argument } [ "," ] ;
argument      = expr | arrow | FIELD "=" expr ;
arrow         = ( IDENT | "(" [ IDENT { "," IDENT } ] ")" ) "=>" expr ;
inline-match  = "match" expr "{" "case" "some" "(" IDENT ")" "=>" expr
                "," "case" "none" "=>" expr [ "," ] "}" ;
```

The `binary` production is precedence-resolved, not a claim that all operators
have the same precedence. From weakest to strongest:

| Level | Operators | Associativity |
| --- | --- | --- |
| Conditional | `? :` | Nested arms parse as expressions |
| Or | `or`, `||` | Left |
| And | `and`, `&&` | Left |
| Equality | `==`, `!=` | Left |
| Comparison | `<`, `<=`, `>`, `>=` | Left |
| Additive | `+`, `-` | Left |
| Multiplicative | `*`, `/`, `%` | Left |
| Prefix | `-`, `not`, `!` | Right |
| Postfix | `.` | Left |

Parentheses override precedence. Comparisons do not create Python-style chained
comparison semantics. Logical operators short-circuit; ternary/match evaluates
only its selected arm. Conditions require booleans.

Named arguments are accepted only by the corresponding constructs: component
uses, record construction/copy, localization (`t`), placeholders (`empty`),
canvas `surface=` bindings, and specialized host commands (`share`,
`scrollIntoView`). Ordinary function calls are positional. Arrow expressions are admitted
as the callbacks to `map` and `filter`, with at most item and index parameters;
they are not first-class values or event handlers.

An app-declared shape's call constructs a record: either all named fields, or one
positional base followed by zero or more replacements. A `list` literal is
a list of its items, left to right; their types meet as a ternary's arms do (`type-list-item` when they do
not), and a trailing comma is kept. There is no spread (`[...xs, x]` is
refused), object literal, method call, bracket indexing, assignment
expression, `??`, `?.`, or `===`. `any` in an internal roster signature describes
special typing; it is not a source-language type annotation.

## Authored tests

```ebnf
test-file     = { launch | test } ;                   (* a top-level launch line: every test's *)
test          = "test" STRING block( { launch } { step } ) ;
launch        = "size" NUMBER "x" NUMBER NL          (* written 1200x800 *)
              | "epoch" ( STRING | NUMBER ) NL       (* "2026-09-21T12:00:00Z" or Unix ms *)
              | "time-zone" STRING NL                (* an IANA zone, "America/New_York" *)
              | "locale" STRING NL                   (* a BCP 47 tag, "fr-FR" *)
              | "seed" NUMBER NL                     (* 0 through 2^53 - 1 *)
              | "before" "data" NL ;                 (* the first step does not wait for data *)
step          = "tap" STRING [ "hover" | "dblclick" | "contextmenu"
                  | "pinch" NUMBER [ "at" NUMBER NUMBER ]
                  | "into" STRING
                  | "modifiers" STRING ] NL
              | "tap" STRING "drag" [ "-" ] NUMBER [ "-" ] NUMBER
                  { ( "press" | "over" | "hold" ) NUMBER
                  | "from" NUMBER NUMBER | "mouse" }
                  [ "during" { STRING } ] NL
              | "type" STRING ( STRING [ "append" ]
                  | "key" STRING [ "down" | "up" | "for" NUMBER ]
                  | "copy" | "cut" | "paste" STRING ) NL
              | "pick" STRING ( STRING { STRING } | "cancel" ) NL
              | "clock" ( "settle" | "data" | [ "+" ] NUMBER [ "real" ] ) NL
              | "resize" NUMBER "x" NUMBER NL        (* the window, mid-test: 800x600 *)
              | "reload" NL
              | "close" NL                           (* the window's close button *)
              | "screenshot" STRING NL
              | "expect" "tree" ( "has" | "missing" ) STRING NL
              | "expect" "text" STRING "==" STRING NL
              | "expect" "state" IDENT { "." ( IDENT | NUMBER ) } "==" test-value NL ;
test-value    = [ "-" ] NUMBER | STRING | "true" | "false" | "none" | "[" "]" ;
```

Targets are driver test ids. Each test is a session of its own, opened with its
launch lines: `size 1200x800`, `epoch "2026-09-21T12:00:00Z"`, `time-zone
"America/New_York"`, `locale "fr-FR"` and `seed 7` are the driver's `--size`,
`--epoch`, `--time-zone`, `--locale` and `--seed`, so they lead the test's steps,
each once. Written at the top of the file they apply to every test that does not
name its own; either way they override the drive's flags. A file whose
assertions depend on the date says so in the file. Before the first step, and
after a `reload`, the driver waits for the app's data as `clock data` does (its
module activated, every request in flight answered and each answer's `then`
landed, the clock unmoved); `before data` skips the wait. `tap "id" drag to "other" [at x y]` ends on the other
node (LLP 1094 D12). `tap "id" drag dx dy` is the driver's `tap … drag` (from the
node's middle, or `from x y` in its box, in points; `press`, `over`, `hold` in
milliseconds; each once). It is a finger where the carrier has one (the web,
iOS: on a simulator the touch runner's real gesture, LLP 1080.000 §11, which a
test with a drag starts for its session; a phone has none yet and fails the
step); `mouse` makes it the left button on the web, with the page's pointer
`fine`, so a desktop path is what runs (iOS refuses it; macOS and Linux drag
with the mouse anyway). A finger's drag the browser takes to scroll an
ancestor ends in `panrelease` and a `pan cancelled` journal line naming the
`touch-action` that keeps it. `during "op" …` is last: each quoted op is a read
(`tree`, `layout`, `state`, `logs`, `screenshot`) or `clock`, run while the finger
is down, after the move and before the hold. `press` and `hold` advance the
virtual clock by those milliseconds (a game's ticks), except under
`--timing platform`, where the platform's own clock owns the gesture.
`tap "id" pinch <scale> [at x y]` is the driver's pinch: `scale` greater than 0,
about the node's middle or about `at` in its box.
`tap "id" dblclick` and `contextmenu` are the driver's forms of the same names
(no Linux carrier double-clicks). `tap "list" into "key"` brings a virtualized
list's row into view by its key, so a row outside the rendered window can be
tapped by its own id on the next step. `type "id" "text"` sets the field's
value, as Playwright's `fill`; `append` adds the text after the value the tree
shows (a prefilled reply). `type "id" key "Name"` presses the key;
`down` and `up` are the two halves, and `for <ms>` holds it that long on the
virtual clock (`down` or `up` together with `for` is refused). `reload` restarts the app on the store it had: the
web page loads again in the same profile, a native app relaunches on the same
scratch store. Its state starts over and the clock is 0 again; what the app
stored is what it reads, so persistence is testable. `close` presses the
window's close button as ⌘W or the red button does (the driver's `close`): its
`beforeunload` handlers hear it, and a window one keeps stays open, so the
test goes on to the app's own "Save changes?" (on the web the browser's "Leave
site?", answered "Stay"). A window that closes, by `close` or by a press the app
answers with `close()`, takes the session with it, and a step after it fails
naming the line. macOS and the web; iOS and Linux close no window and refuse it.
`type` on a `select` chooses an enabled option by value, else by its one label;
on a date, time or range input it sets the value in HTML's format; on a checkbox
it takes `true` or `false`. A target out of view is scrolled into view first.
An input step ends with what it settled (an answer given in the input's turn,
and its mutation's `then`); otherwise the clock stands still between steps:
what an input starts (a reply on real time, a transition) lands at a `clock`
step, as `clock settle` (or `clock data`, which lands replies and their `then`s
without moving the clock); a timer fires when the clock reaches or passes its time
(`clock +N`), and `clock settle` fires one only if it reaches it while advancing
to a motion's end.
`expect text` reads the node's text, else a control's value (a `select`'s chosen
value, not its options), else its descendants' text in order (a button's label),
else a field's value. `expect state name.field` reads a field of a record at any
depth, and `name.0` a list index (a whole number from 0); a missing field fails
naming the fields there. The value is a number, including a negative (`== -3`),
a string, a bool, `none`, or `[]` — not an expression (`-1 + 2` is refused).
The test compiler emits steps as JSON; the agent driver executes them. The
nine-operation interactive API is larger than this test-file grammar.

## Standard functions and intrinsics

The complete current `stdlib` name inventory is below. `T` and `U` express generic
relationships enforced by the type checker, not user-declarable type parameters.
Signatures are authored forms; localization's internal lowered signature differs.
An app's `fn` of one of these names shadows it in every expression of the app, as a
JavaScript function declared over a global does: a name the inventory gains later
never breaks an app that declared it first.

| Call | Result / restriction |
| --- | --- |
| `now()` | Milliseconds on the runner's clock since boot (the driver's clock under the agent), not a date: the date is `exactTime().epochAtZero + now()`. A read does not schedule a render: a derive that reads it changes when an action runs or an answer lands, not as time passes, so a value that follows the clock needs a timer (`task … every`), whose ticks re-read it |
| `formatTime(ms, offsetMinutes, "short")` | String; fixed offset east of UTC, en-US formatting (`exactTime().utcOffset` is the zone's offset now, answered again when it changes) |
| `formatDate(ms, offsetMinutes, "medium" or "month-year")` | String; format is a literal choice, not an expression containing `or` |
| `formatNumber(n, "compact")` | String; admitted deterministic compact format |
| `length(value)` | Number; list item count or string UTF-16 code-unit count |
| `isEmpty(value)` | Boolean; empty string or list |
| `toString(value)` | String; number, boolean, or string conversion |
| `floor(n)` | Number |
| `max(a, b)` | Number |
| `min(a, b)` | Number |
| `includes(text, substring)` | Boolean, case-sensitive literal substring |
| `includes(list<T>, item)` | Boolean; `T` a string, number or bool, compared by SameValueZero (`NaN` is found, `-0` is `0`) |
| `startsWith(text, prefix)` | Boolean, case-sensitive |
| `endsWith(text, suffix)` | Boolean, case-sensitive |
| `trim(text)` | String, JavaScript whitespace/line-terminator trimming |
| `slice(text, start, end?)` | String; JavaScript's `slice` over UTF-16 code units: fractions truncate, NaN is 0, a negative index counts from the end, an omitted `end` is the end; a cut surrogate half is U+FFFD |
| `slice(list<T>, start, end?)` | `list<T>`; the items, the indices clamped as text's are |
| `concat(list<T>, list<T>)` | `list<T>`; both lists' items in order (`[...xs, x]` is `concat(xs, [x])`) |
| `indexOf(text, substring)` | Number; the first match's position in UTF-16 code units, `-1` for none, `0` for `""` |
| `indexOf(list<T>, item)` | Number; `T` a string, number or bool, compared by `===` (`NaN` is never found, `-0` is `0`); `-1` for none |
| `split(text, separator)` | `list<string>`; the pieces between the separator's matches, left to right; `""` splits into UTF-16 code units (a cut surrogate half is U+FFFD), and `split("", "")` is `[]` |
| `replaceAll(text, find, with)` | String; every match of the string `find`, left to right; `$$`, `$&`, `` $` ``, `$'` in `with` (`$1` is literal); an empty `find` inserts at every code-unit boundary; no regular expressions |
| `toLowerCase(text)` | String; Unicode's default lowercase, final sigma kept, no locale (the web core links the case tables by use) |
| `first(list<T>)` | `option<T>` |
| `at(list<T>, index)` | `option<T>`; truncates index toward zero, negative from end |
| `map(list<T>, callback)` | `list<U>`; callback returns one value |
| `filter(list<T>, callback)` | `list<T>`; callback returns boolean |
| `join(list<string/number/bool>, separator)` | String; primitive item printing |
| `encodeURIComponent(text)` | String |
| `encodeRouteSegment(text)` | String; routing's checked segment encoding |
| `open(router, location)` | Router |
| `push(router, location)` | Router; same top URL does not add a visit |
| `replace(router, location)` | Router |
| `back(router)` | Router |
| `select(router, tabName)` | Router |
| `go(router, location)` | Router |
| `stack(router)` | `list<Entry>` |
| `top(router)` | `Entry` |
| `depth(router)` | Number |
| `params(router, parameterName)` | `list<string>`: that parameter's non-empty values over the selected stack, bottom first, duplicates kept (one entry's own value is `entry.params.<name>`, `""` where its route does not bind it; the name must be a parameter of some route) |
| `searchParam(entry, name)` | String |
| `t("key", name=value, …)` | Localized string; validates tables/placeholders |
| `frame(id)` | `Geometry`, actions only; last layout where the viewer sees it, as `getBoundingClientRect`: in the viewport, every scroll offset and transform applied |
| `measure("id")` | `Geometry`, actions only; literal id, height-auto measurement |
| `elementFromPoint(x, y)` | `option<string>`, actions only; the `id` of the front-most of `frame`'s boxes at the viewport point, or of its nearest ancestor with one (DOM's `elementFromPoint(x, y)?.closest("[id]")?.id`): ancestors' overflow clips apply, `pointer-events: none` and hidden boxes are passed over |

The router functions (`open` through `searchParam`) and `encodeRouteSegment`
exist only in an app with a `routes` declaration.

`<`, `<=`, `>` and `>=` take two numbers or two strings. Two strings compare as
JavaScript's do, by UTF-16 code units in order with a proper prefix first, no
locale (`"09:30" < "10:00"`, `"Z" < "a"`). `slice` and `replaceAll` work on code
units as JavaScript does and make the whole result well formed once: a lone
surrogate half is U+FFFD, since the native runners hold Unicode scalar values.
A result past the runner's string bound (64 MiB of UTF-8) traps on every
executor. `toUpperCase`, `padStart` and number parsing are not in Contract;
the refusals say what to write instead. Neither `indexOf` nor `split` takes
the web's second argument (a start position, a limit), and `split` takes no
regular expression.

`concat`, `split`, and `slice`, `includes` and `indexOf` over a list, take
list steps on the evaluation's budget as `map` and `join` do (LLP 1090): one
for each item `concat`, `slice` or `split` keeps, taken before the list is
built, and one for each item `includes` or `indexOf` scans up to its match. A
list they build is bounded as any built value is.

Compiler intrinsics and special forms additionally include:

| Form | Meaning |
| --- | --- |
| `pending(resourceOrMutationName)` | Boolean; request is in flight |
| `failed(resourceName)` | Boolean; current resource request failed without an answer |
| `path("routeName", args…)` | Checked encoded route location |
| `empty(field=constant, …)` | Resource placeholder's zero record with overrides |
| `some(expr)` | Construct an option |
| `DeclaredShape(field=value, …)` | Construct a record |
| `DeclaredShape(base, field=value, …)` | Copy a record |

`Router`, `Tab`, `Entry`, and `Params` are introduced by routes. The compiler
also declares `Geometry` (geometry reads), `MarkdownSelection` (the `select`
payload) and `Picked` (a file input's `change` payload). None of these is
constructible. The actual sources are [`format.json`](../plan/tables/format.json),
[`runner/src/stdlib.rs`](../runner/src/stdlib.rs), and the compiler's type/lowering
modules. A function available in JavaScript, CSS, Swift, or a data module does
not automatically become a Contract function.

## Built-in tags

The compiler's built-in tag vocabulary is grouped here by use. It is not an
HTML parser: arbitrary HTML names outside this table are not implicitly accepted.
Several tags share a kernel node type with different fixed properties.

| Family | Names |
| --- | --- |
| Boxes / layout | `view`, `box`, `row`, `column`, `scroll`, `list` |
| Structure | `main`, `header`, `nav`, `section`, `footer`, `article`, `aside`, `dialog`, `hr` |
| Text and controls | `text`, `button`, `link`, `input`, `textarea`, `select`, `option` |
| Media / metadata | `image`, `video`, `audio`, `iframe`, `canvas`, `head` |
| SVG scene | `svg`, `g`, `path`, `polyline`, `polygon`, `circle`, `ellipse`, `line`, `rect` |
| SVG definitions | `defs`, `symbol`, `use`, `clipPath`, `marker`, `mask`, `pattern` |
| SVG color / text / embedding | `linearGradient`, `radialGradient`, `stop`, `tspan`, `foreignObject` |
| SVG filters | `filter`, `feBlend`, `feColorMatrix`, `feComponentTransfer`, `feComposite`, `feConvolveMatrix`, `feDiffuseLighting`, `feDisplacementMap`, `feDropShadow`, `feFlood`, `feFuncR`, `feFuncG`, `feFuncB`, `feFuncA`, `feGaussianBlur`, `feMerge`, `feMergeNode`, `feMorphology`, `feOffset`, `feSpecularLighting`, `feTile`, `feTurbulence`, `feDistantLight`, `fePointLight`, `feSpotLight` |

`text` inside SVG has SVG semantics. A hyphenated tag names a native module: the
bake checks it against `app.json`'s `modules` list (`bake-unknown-module`), and
its unknown attributes, and an SVG element's own props, pass to the module as one
object. A known attribute binds to the module's box, and the box takes only layout,
box and paint rows, handlers, `testId`, `id`, `class`, `data-*`, `role`, ARIA,
`disabled` and `inert`; any other known name (`color`, `value`, `command`, `href`)
is `lower-native-attr` (LLP 1024 D1, LLP 1088 D7.2). A capitalized name is a component use, not a
built-in tag. Platform support can further restrict an admitted tag, notably
native `foreignObject`.

`contract vocab` lists every tag, style and prop name the compiler admits,
with each style row's codec, values and default, the renamed spellings and the
contextual restrictions: `cargo run -q -p contract -- vocab padding` for one
name, no name for all, `--json` for a document. From an app made by
`exact new`, run `bun exact.mjs contract vocab`.
`padding`, `margin`, `inset`, `border-width`, `border-style` and `border-color` take
CSS's one to four values (`padding="12px 40px"`: top and bottom 12, sides 40), and
`border-radius` its one to four corners (`border-radius="18px 18px 0 0"`: top-left,
top-right, bottom-right, bottom-left; no `/` elliptical radii).
CSS hyphens are part of the authored name. `testId` and admitted host-specific
props retain their declared spelling.

A `button` is Chrome's `<button>` with Exact's reset ([LLP
1001](../llp/1001-kernel-v1.spec.md) §1): a block that shrinks to fit, whose
content is centred in its height (safely: content taller than the button
starts at the top) and whose text is `text-align: center`. `align-items`,
`justify-content` and `gap` do nothing on it, as on any block. Write
`display="flex"` (a row, CSS's default) or `display="grid"` to lay its
children out yourself; Chrome does not centre a flex or grid button's content.
Write `text-align="start"` for a row- or card-like button whose text reads
from the left. Declared: it is block-level, not `inline-block`, so buttons in
a block parent stack (put them in a `row` to set them side by side).

A link (`link href`, a text run's `href`, a Markdown link) to a path in the
app (`/note/3`, one of its declared routes) navigates in it, through the
navigation root's `navigate`, on every host; a path that names no route (a
file beside a document) is the browser's, or natively the app's document to
open. A relative path (`note/3`) has no base natively; write it from `/`. One to an absolute URL (`https://…`, `//…`) leaves the
app: natively it opens in the system browser (the Linux host has none and
logs it), and on the web in a new
browsing context (`target="_blank" rel="external noopener"`), so the app is
still there when the reader comes back. On the web, `target="_self"` keeps
such a link in the page (it replaces the app) and `target="_blank"` opens a
path in a new one; natively `target` changes nothing, an app having no
other tab. `mailto:` and `tel:` links go to their handlers everywhere.

`button appearance="auto"` selects a native control; the literal switch is
resolved after class merging. Default/`none` keeps the authored pressable.
Native face content, styles, transitions/keyframes, and enclosing contexts have
an explicit allowlist in [`controls.rs`](../contract/lower/src/controls.rs).
`buttonStyle` is a styleable host-policy prop whose complete vocabulary is the
schema's `buttonStyles` table; default `bordered`. This adds no tag or new
expression grammar. Platform looks and stand-ins are documented in
[LLP 1069.011](../llp/1069.011-native-buttons.rfc.md).

### Markdown: `markup`, `format`, `select`

`markup="markdown"` on a `text` or a `textarea` styles the node's own string as
Markdown ([LLP 1045](../llp/1045-markdown-editor.rfc.md)); `none` is the
default. It is not CSS: CSS has nothing for this. The value stays the Markdown
source, so storage, search and export are the string.

- `text note.body markup="markdown"` is the reader: headings, lists, quotes,
  code and links painted as one node, on every host.
- `textarea id="editor" value=draft input=edit markup="markdown"` is the editor:
  on the web, Markdown shown as it reads, with the syntax hidden away from the
  caret; on macOS and iOS, the platform's text view with the syntax dimmed. Linux
  edits it as a plain textarea. Without `markup` the same source is a plain
  textarea: the source mode.

`format(id, command)` and `format(id, command, argument)` edit the editor whose
`id` is given, at its selection, as one undo step reported through its `input`
as typing is. The commands are `bold`, `italic`, `code`, `strike`, `link` (the
argument is the URL), `heading` (`"1"` to `"6"`), `bullet`, `ordered`, `task`,
`toggleTask`, `quote`, `codeblock`, `footnote`, `figure` (the argument is the
media `src`), `indent`, `outdent` and `newline`; another name, or a missing
argument, changes nothing. A toolbar button takes `retainFocus=true`, so a press
leaves the editor focused and its selection where it was.

The editor's `select` hands its action a `MarkdownSelection` when the selection
or the source changes: `formats`, space-separated (the inline command names,
`heading1` to `heading6`, `bullet`, `ordered`, `task`, `quote`, `codeblock`);
`mixed`, true when the selection spans different formats or links; `link`, the
common link target or `""`; and `unavailable`, the commands that cannot apply
there, space-separated (most of them, inside a code block). No offsets cross.
A toolbar reads one format as `includes(` ${s.formats} `, " bold ")`;
[Markdown Stress](../apps/markdown-stress/app.contract) is a whole toolbar and
link sheet.

## Events

An event binding is an action reference or partially applied action. Captured
arguments precede the event payload. The table contains all 51 handler names.
Numeric multi-argument payload ordering should be copied from the feature's
working fixture, not inferred from JavaScript's Event interface.

| Payload appended to captured arguments | Handler names |
| --- | --- |
| One string | `change`, `input` (text field, textarea, `select`), `message`, `error` |
| A string, then optionally a `KeyboardEvent` | `key`: the key's name; an action taking one more parameter also hears the [modifiers](#keys) |
| Two numbers, then optionally a `ScrollEvent` | `scroll`: left and top; an action taking one more parameter also hears the scroller's extents (below) |
| Two numbers, then optionally a `DOMRectReadOnly` | `resize` given an action: the content box's width and height; an action taking one more parameter also hears its `contentRect` (below). A string `resize` is CSS's property |
| One boolean | `hover`; `change`, `input` on a checkbox or `switch` |
| One number | `timeupdate`, `durationchange`; `change`, `input` on `type="range"` |
| One `list<Picked>` | `change`, `input` on `type="file"` |
| One `MarkdownSelection` | `select` |
| Two numbers | `pan`, `panrelease`, `heightrelease` |
| A string, an `option<string>`, then optionally a `ReorderEvent` | `reorderdrop`, on a vertical `list virtualized=true` only: the dragged row's key, then the key it lands before (`none` at the end); an action taking one more parameter also hears `ReorderEvent { from, to }`, the two lists' `id`s (equal within one list; [LLP 1094](../llp/1094-dropping-across-lists.rfc.md) D2) |
| Four numbers | `transformgeometry` |
| Six numbers | `transformrelease` |
| Special: zero or one location string, no captured args | `navigate` |
| Zero or one `PointerEvent` (the action takes it or leaves it) | `pointerdown`, `pointerup`, `pointermove`, `contextmenu` (UI Events makes it one: where the secondary click or long press was; a keyboard's menu key gives the origin) |
| Zero or one `WheelEvent` (the action takes it or leaves it) | `wheel` |
| Zero or one `DragEvent` (the action takes it or leaves it) | `drop` |
| Zero or one `ClipboardEvent` (the action takes it or leaves it) | `copy`, `cut`, `paste` ([clipboard](#clipboard)) |
| Zero or one `Selection` (the action takes it or leaves it) | `selectionchange`, on a `text` ([text selection](#text-selection)) |
| Zero or one `MouseEvent` (the action takes it or leaves it) | `press`: the modifier keys held, `shiftKey`, `ctrlKey`, `altKey`, `metaKey` (a shift-click, a ⌘-click; all false from a keyboard or assistive activation) |
| None | `beforeunload`, `cancel`, `focus`, `blur`, `submit`, `load`, `dblclick`, `swiperight`, `refresh`, `loadedmetadata`, `play`, `playing`, `pause`, `ended`, `waiting`, `seeking`, `seeked`, `ratechange`, `volumechange`, `canplay`, `reachstart`, `reachend` |

An `audio` is HTML's: `video`'s props and events without `poster`, `playsinline`
or `playbackVisibilityThreshold`; no box unless `controls` (then Chrome's 300×54,
which `width`/`height` override), whatever `display` says
([LLP 1042](../llp/1042-video.spec.md) §8).
A `video`'s or `audio`'s `error` appends a stable code, never the engine's text: MediaError's
`aborted`, `network`, `decode` and `src-not-supported` (a source that never loaded),
`not-allowed` (the browser refused to start playing) or `invalid-value` (a number out
of range). A play interrupted by a pause or a new source is no error. A `video` the
tree removed reports nothing more, on every host
([LLP 1042](../llp/1042-video.spec.md) §3).
`scroll` appends left then top offsets, and to an action that takes one more
parameter a `ScrollEvent`: the scroller's own `scrollLeft`, `scrollTop`,
`scrollWidth`, `scrollHeight`, `clientWidth` and `clientHeight` as the event
fires, what a web handler reads off `event.target`. "At the end" is the web's
arithmetic, on every host and on a virtualized list too (chat F4, a
jump-to-latest pill); a native host's `scrollHeight` is its port plus the range
it clamps to:

```text
action moved(x: number, y: number, e: ScrollEvent)
  away = e.scrollHeight - e.scrollTop - e.clientHeight > 1
list virtualized=true scroll-start="end" scrollFollowEnd=true scroll=moved …
```

As on the web, the event comes when the offset changes (a follow of the end
moves it); content that grows below a reader who is not following it changes
no offset and sends none.

`resize` is two things, told apart by its value, as nowhere else: a string
(`resize="none"`) is CSS's `resize` property, and an action is the element
resize event, `ResizeObserver`'s. Its action hears the content box's width
and height after layout: once the element is first laid out, then whenever
its content box changes size (a commit that leaves it alone says nothing; one
under `display: none` reads 0 by 0). One more parameter hears the entry's
`contentRect`, a `DOMRectReadOnly`: `x` and `y` the padding's left and top,
`width`, `height`, `top`, `right`, `bottom`, `left`. The web observes with
the browser's `ResizeObserver`; a native host delivers after its layout and
lays out again before the next round, each round only to elements deeper than
the shallowest the last one reached, as the browser's loop does, so a handler
that keeps growing its own box cannot spin: what is left is delivered after
the next layout and the log says `ResizeObserver loop completed with
undelivered notifications`. Geometry inside the action is `frame(id)`:

```text
action fit(w: number, h: number)
  columns = floor(w / 240)
column resize=fit …
```

`pan` supplies the move since its last event (dx, dy),
the first one since the press, so their sum is the drag's offset from the press. `panrelease` appends x/y release velocity;
`heightrelease` appends height and velocity. A `pan` hears a drag that starts
anywhere inside it, a nested `button` or `press` node included: past the slop
the pan takes the contact and the press does not fire, while a tap still
presses. A nested text input or control keeps its own drags, and the innermost
recognizer wins ([LLP 1057.001](../llp/1057.001-gesture-precedence-and-pinch.spec.md) §1).
The compiler validates arity and
available payload types; tags and hosts constrain where events make sense.
`navigate` belongs on the first root element, outside any region, which must
also carry `navigationKey` and `navigationBack`. Transform geometry/release
bindings are required as a pair. See
[`handler_arity`](../contract/analyze/src/lib.rs) and the corresponding corpus/tests.

### Pointer

`pointerdown`, `pointerup` and `pointermove` are DOM's, on every host. The
innermost enabled node under a touch or a button (any: `buttons` says which, 2
for a right-click) that hears any of them holds the pointer: its `pointerdown` fires before any gesture decides,
its `pointermove`s follow the pointer wherever it goes while held, and its
`pointerup` comes when the pointer lifts or is cancelled (a cancel is an up),
before the click's `press`. A free pointer (a mouse or a pen hovering, no
button down) moving over a node is that node's `pointermove` too, the
innermost hearing it; a touch has none. Moves go out at most once a frame,
the latest. None of them takes anything from `press`, `pan` or scrolling, so
a drawing surface sets `touch-action="none"`, as on the web, or a touch
that scrolls is cancelled.

An action that takes one more parameter than the binding captures gets a
`PointerEvent` (DOM's names, [LLP 1056](../llp/1056-canvas-2d.rfc.md) §8.6):

| Field | Meaning |
| --- | --- |
| `offsetX`, `offsetY` | The point from the node's content box, CSS px (DOM measures from the target's padding edge; a canvas draws in its content box) |
| `buttons` | DOM's bits: 1 primary, a touch or a pen in contact; 2 secondary; 4 auxiliary; 0 on `pointerup` and a hover |
| `pressure` | 0 to 1: a pen's or a pressed touch's force where the platform measures one, else 0.5 while down and 0 while not |
| `pointerType` | `mouse`, `pen` or `touch` |
| `pointerId` | 1 for the mouse; a touch or pen has its own while down |
| `clientX`, `clientY` | The point from the viewport, CSS px: `frame()`'s space, so a hit test against `frame` needs no scroll bookkeeping ([LLP 1094](../llp/1094-dropping-across-lists.rfc.md) D11) |
| `shiftKey`, `ctrlKey`, `altKey`, `metaKey` | The modifier keys held (a hardware keyboard's, on iPadOS) |

```text
action stroke(e: PointerEvent)
  points = `${points} ${e.offsetX},${e.offsetY}`
canvas surface=ink(points) pointerdown=begin pointermove=stroke touch-action="none"
```

A `contextmenu` (a right-click, a long press) offers the same record: on a Mac
and in a browser on one it comes on the button's down, after its `pointerdown`.

`wheel` is DOM's: a wheel's turn or a trackpad's scroll over the node, heard by
every node from it up that declares it, innermost first. Its `WheelEvent` is
`offsetX`, `offsetY`, `deltaX`, `deltaY` (CSS px; positive scrolls down and
right), `deltaMode` (0 pixels) and the four modifiers. A trackpad's pinch is a
wheel with `ctrlKey` and `deltaY` of -100 × its magnification, as browsers
deliver one. An action that calls `preventDefault()` keeps the scroll from
happening (web, macOS, Linux; iOS has none):

```text
action wheeled(e: WheelEvent)
  if e.ctrlKey or e.metaKey
    zoom = zoom * (1 - e.deltaY / 100)
    preventDefault()
scroll wheel=wheeled …
```

`drop` is DOM's `drop` of files dragged in from outside the app (Finder, the
desktop) onto the innermost node that declares it: its `DragEvent` is
`offsetX`, `offsetY`, `files` (a `list<string>` of `doc:` handles minted as a
picker's are, [LLP 1069.010](../llp/1069.010-the-mac-as-a-document-platform.rfc.md)
D1, readable under `fs.read doc:/`) and the modifiers. Only files of the types
the manifest's `file_handlers` declares are taken; another is refused into the
journal. Web (a browser without `getAsFileSystemHandle` hands a read-only copy)
and macOS; iOS and Linux have none. The driver's `tap <target> drop <path…>`
drags files in.

`beforeunload` is DOM's: the window is about to close or the app to quit
(macOS: its close button, File ▸ Close Window, ⌘Q; the web: leaving the page).
Every element that declares it hears it; an action that calls `preventDefault()`
keeps the window open. The browser then asks "Leave site?" itself; a Mac app asks
its own question, and calls the host command `close()` once it is answered,
which closes the window without asking again (on the web, `window.close()`, which
a browser honours only for a window a script opened). iOS and Linux close no
window. `head edited=…` marks the document as unsaved: on macOS the dot in the
window's close button, beside the proxy icon of the file the window opened;
other hosts show nothing (a declared deviation: the web has no unsaved mark).

HTML's global `title` attribute, on any element but `head` (whose `title` is the
document's), is advisory text: the browser's tooltip on the web, `toolTip` on
macOS; touch hosts show none.

### Clipboard

`copy`, `cut` and `paste` are DOM's, on every host: ⌘C, ⌘X and ⌘V (Control on
Windows and Linux keyboards, the Edit menu, an iPad's hardware keyboard) with
the focus at a node or inside it are heard by the nearest node with the
handler, itself or an ancestor — so a node with one takes the focus, as a
`key` node does. An action that takes one more parameter gets a
`ClipboardEvent` whose `text` is the clipboard's plain text: what is pasted,
and empty on `copy` and `cut`, as the DOM's is until a listener sets it — the
action writes the clipboard with `copyText`. A field's own paste still
inserts the text. On macOS and iOS, a text field's or textarea's editing is
the platform's and fires none of the three (the web's fires them); the
driver's `type <id> paste <text>` delivers a paste carrying that text, and
`type <id> copy` and `type <id> cut` the others, without touching the
system clipboard.

```text
action pasteAt(cell: string, e: ClipboardEvent)
  send pasted = pasteCells(cell, e.text)
column key=move paste=pasteAt(selected) copy=copyCells cut=cutCells
```

### Text selection

`selectionchange` on a `text` is the web's `selectionchange`, per element:
when the part of the reader's text selection inside that paragraph changes —
a drag, a double or triple click, select-all, a click that clears it — an
action that takes one more parameter gets a `Selection`: its `text` (the
selected part, as `Range.toString()` gives it) and its `start` and `end`,
UTF-16 offsets into the node's own text as written (white space before CSS
collapses it, a `text`'s inline children joined in order). Nothing selected
there is `text: ""` with `start == end == 0`; it fires while a drag moves,
once per change. Web and macOS select text; iOS and Linux have no text
selection on a `text`, so it never fires there.

```text
action mark(para: string, s: Selection)
  selection = Excerpt(para=para, from=s.start, to=s.end, text=s.text)
text para.body selectionchange=mark(para.id)
```

### Keys

`key` is the DOM's `keydown`, on every host (web, macOS, iOS and iPadOS with a
hardware keyboard, Linux):

- **Where.** The key goes to the focused element: a field or textarea being
  edited, a `button`, or any element with a `press`, `focus`, `blur` or `key`
  handler (such an element takes the focus, as `tabindex="0"` gives it, and
  is in the Tab order), or with a `tabindex`. It then bubbles: the
  focused element's handler hears it first, then every ancestor's, innermost
  first. With nothing focused, only `aria-keyshortcuts` buttons hear keys.
- **What.** The payload is `KeyboardEvent.key`: the character typed, Shift's
  included (`"a"`, `"A"`, `"7"`, `" "`, `"/"`), or the key's name (`"Enter"`,
  `"Escape"`, `"Tab"`, `"Backspace"`, `"Delete"`, `"ArrowUp"`…, `"Home"`,
  `"End"`, `"PageUp"`, `"PageDown"`, `"F1"`…, `"Shift"`). Every key is heard,
  printable ones in a field included. Keys an input method is composing are
  its own.
- **Modifiers.** An action that takes one more parameter, typed
  `KeyboardEvent`, hears the event too: the record `{ key: string, shiftKey:
  bool, ctrlKey: bool, altKey: bool, metaKey: bool }`, the DOM's fields
  (`altKey` is Option and `metaKey` Command on a Mac). A key typed with
  Control or Meta held is a shortcut: it types nothing.
- **Then the default.** After the handlers, the key does what it would have:
  a character is typed into the focused field, Backspace deletes, Enter
  commits an input (its `change`, when its value changed, as HTML's does)
  and then submits it (`submit`), breaks a textarea's line (a textarea has no
  `submit`, as in HTML) or presses a button, Space presses a button (Enter
  and Space press any element with a `press` handler as they do a button,
  Enter alone a `role="link"`; give it `role="button"` to be announced as one),
  Tab moves the focus, arrows move the caret; on the web arrows, Space and
  the page keys also scroll the page or the focus's scroller (a native
  scroller does not scroll by key, so there is nothing there to prevent).
- **Claiming a key.** An action run by a `key` event that calls the host
  command `preventDefault()` is the handler's `event.preventDefault()`: that
  default does not happen. Ancestors' handlers still hear the key, as they do
  on the web, unless the action also calls `stopPropagation()`, the handler's
  `event.stopPropagation()`: no ancestor's `key` handler hears it, and its
  default still happens (an inline rename field's Enter submits without the
  list around it opening the selection). Call either only for the keys you
  handle, so typing still works:

```text
action move(k: string)
  if k == "ArrowDown"
    cursor = cursor + 1
    preventDefault()
```

Enter sends and Shift+Enter breaks the line, as a chat composer does (on a
phone the software keyboard's Return is Enter, so it sends there too):

```text
action compose(k: string, e: KeyboardEvent)
  if k == "Enter" and not e.shiftKey
    send(draft)
    preventDefault()
```

`textarea value=draft input=write key=compose`. A shortcut reads the modifier
the platform's users press: `(e.metaKey or e.ctrlKey) and k == "s"` saves on a
Mac and elsewhere. The driver presses chords in Playwright's spelling (`type
"composer" key "Shift+Enter"`, `key "Meta+s"`).

- **A game's canvas.** A key at a canvas whose world takes input, or at a
  node inside one, goes the same way first: a shortcut takes it, then the
  `key` handlers from the focus out hear it, and one that calls
  `preventDefault()` keeps it from the world. The world hears the rest, so a
  canvas's `key=` handler hears what the world binds as well as what it does
  not (the platformer's diary, R8).
- **Shortcuts.** An `aria-keyshortcuts` button hears its chord before any
  `key` handler, and takes the key (no `key` handler hears it), on the web,
  macOS, iPadOS (a hardware keyboard's chord; the session's view holds
  the focus when nothing else does) and Linux. While a modal is shown — a modal
  `dialog`, or an `aria-modal` view, the last shown — only the buttons
  inside it hear their chords, and Enter or Space with the focus on a
  control they activate (a button, a pressable, a checkbox) is that
  control's, whatever button declares it. A chord's key is one character,
  a named key, or F1–F35; the paste chord (⌘V, Control+V off the Mac) is a
  chord like any other, so a button declaring it takes the driver's `type
  … paste` too and the paste never lands. On iPadOS a hardware keyboard's
  F13–F24 reach `key` handlers but no shortcut (UIKit's key commands name
  F1–F12 only); the driver's reach both, as everywhere.
- **The Mac's menu bar.** Every button whose chord holds ⌘ is also a menu
  item, titled by its `aria-label` (or its text), placed by its chord as
  Apple's HIG places one: ⌘, is Settings…; ⌘[ ⌘] and a `tablist`'s tabs are
  Go; ⌘Z ⇧⌘Z ⌘X ⌘C ⌘V ⌘A ⌘D ⌘F ⌘G ⇧⌘G are Edit; ⌘= ⌘+ ⌘- ⌘0 and any ⌃⌘
  chord are View; the rest are File. One whose chord is the host's own Edit
  item's (Undo, Redo, Cut, Copy, Paste, Select All) takes that item's place,
  so Edit ▸ Undo is the app's "Undo Move"; any other host item whose chord a
  button declares keeps its place without the chord (File ▸ Close Window
  beside the app's ⌘W). Drop the chord while a field is being edited and the
  host's text Undo, Cut, Copy and Paste come back (studio diary R16).

### Focus order: `tabindex`

HTML's `tabindex` (no `tabIndex` alias; [LLP 1088](../llp/1088-what-the-app-diaries-ask-of-contract.rfc.md)
D7.3), on any element and a module tag's box, a number or bound to state. Present, it
makes the element focusable — a click, `focus(id)` and `autofocus` reach it, and the
`key` handlers above it hear its keys; absent is never `0`, so a plain box is no stop.
`tabindex >= 0` is a Tab stop: positive values first in ascending order, then `0` and
the elements that are stops by kind (inputs, buttons, handlers) in tree order. A
negative value is focusable but skipped by Tab, a handler's element included
(`button tabindex=(revealed ? 0 : -1)` keeps a hidden swipe action out of the order).
An inert or `display: none` element is never focusable, nor is a disabled `button`,
`input`, `textarea` or control. `disabled` means nothing on a box, as on a `<div>` in
Chrome: `box tabindex=0 disabled=true` is still a Tab stop, and so is a `link` with
an `href` (the web's `<a>`, which `disabled` does not touch). Tab and Shift-Tab walk
and wrap on the web, macOS, iPadOS's hardware keyboard and Linux; tvOS's remote skips a
negative value and keeps UIKit's geometric order.

## Host commands

The current command name inventory is:

`blur`, `copyText`, `deliveryActivate`, `deliveryCheck`, `focus`, `format`,
`openURL`, `selectText`, `setScheme`, `showPicker`, `share`, `saveFile`,
`showOpenFilePicker`, `showDirectoryPicker`, `showSaveFilePicker`, `scrollIntoView`,
`showNotification`, `closeNotification`, `haptic`, `postMessage`, `reload`, `close`
([pointer](#pointer): a window's `beforeunload`), `preventDefault` and
`stopPropagation` ([keys](#keys)).

These appear only as action statements. They are not ordinary value-returning
functions. Some have dedicated compiler checks while others also rely on host
argument validation. Use the working implementation when selecting arguments:

| Operation | Starting point |
| --- | --- |
| `focus(id)` | [Markdown Stress](../apps/markdown-stress/app.contract) |
| `format(id, command[, argument])`: a Markdown editor's toolbar command ([Markdown](#markdown-markup-format-select)) | [Markdown Stress](../apps/markdown-stress/app.contract) |
| `blur()`, `blur(id)` | [Messages](../apps/messages/app.contract), [keyboard-bar corpus](../contract/corpus/keyboard-bar.contract) |
| `selectText(...)` | [Messages Legacy](../apps/messages-legacy/app.contract) |
| `copyText(text)` | [Messages](../apps/messages/app.contract) |
| `openURL(url)` | No Contract fixture; the hosts' dispatch, such as [`host/web-js/commands.js`](../host/web-js/commands.js) |
| `setScheme(...)` | [Caltrain](../apps/caltrain/app.contract), [Markdown](../apps/markdown/app.contract) |
| `share(...)` | [share corpus](../contract/corpus/share.contract) |
| `showNotification(title=, body=, tag=, showTrigger=)`, `closeNotification(tag)`: a local notification by the Notification API's names, now or at `showTrigger` (epoch milliseconds); a newer one with the same `tag` replaces it, and `closeNotification` takes it away, shown or waiting. Needs the grant `device.notifications <strings key>`; see [notifications](reference.md#notifications) | [notify corpus](../contract/corpus/notify.contract) |
| `showPicker(id)`, export `saveFile(id, from, suggestedName)`: the host copies the `app:/` file `from` to where the person chooses; `change` at `id` carries the chosen name, `cancel` a dismissal. `saveFile(id, text=…, suggestedName=…)` saves the text itself (UTF-8), no file written first and no grant, so "export what's on screen" is one press | [picker tests](../contract/cli/tests/it/picker.rs), [Fieldnotes](../apps/fieldnotes/app.contract), [Linux's save tests](../host/linux/src/presenter/save_tests.rs) |
| `showOpenFilePicker(id[, multiple])` | [file-picker corpus](../contract/corpus/file-pickers.contract) |
| `showDirectoryPicker(id)` | Same corpus |
| `showSaveFilePicker(id, suggestedName)` | Same corpus |
| `scrollIntoView(id, block=, inline=, behavior=)`: `Element.scrollIntoView()` on any element by its `id` (a string, dynamic as `focus`'s): every scroll container above it, innermost first, then the page, align it by the web's `ScrollIntoViewOptions` (`block` default `start`, `inline` `nearest`). `scrollIntoView("list-id", key, …, row=)`: a virtualized list's row by key, built and measured first (LLP 1070.000). Native hosts land `smooth` at once on the element form | [collection tests](../contract/cli/tests/it/collection_into_view.rs) |
| `deliveryCheck`, `deliveryActivate` | [delivery corpus](../contract/corpus/delivery.contract) |

The web (its JS target) and the Apple hosts carry every command. The
headless Linux host has no browser, clipboard, editor or dev menu: its
`openURL`, `copyText`, `format`, `reload` and `close` are journaled as
unsupported there, `haptic` does nothing, and `showNotification` is refused
(`showNotification: refused: unavailable`). Its `selectText` focuses the field
with its whole text selected, which the next typed key or Backspace replaces.
iOS closes no window either: its `close` is journaled as unsupported. On the web,
`reload()` is the page's own reload, and `deliveryCheck` and
`deliveryActivate` find nothing (a web build has no update store; the page is
the newest root).

Element-targeted commands use `id`, not `testId`. File pickers publish handles
through the target's `change` event; cancellation uses `cancel`. Permissions,
accepted file types, app identity, and platform support remain part of the app
integration. A recognized command name alone does not prove those prerequisites.

## Semantic restrictions

Syntax is only the first layer. In particular:

- Types are closed; unknown/unresolved types, cycles, and invalid property values
  are refused. `none` and `[]` need enough context to infer their contained type.
- Record construction requires all fields once; copying requires a matching base.
  Shapes are finite and nonrecursive. Equality compares values structurally.
- Function bodies are effect-free, nonrecursive expressions (they may read
  `now()`); app functions cannot shadow
  roster names or take a declared shape's constructor name.
- Action reads see the starting snapshot. Writes land together. Locals are
  immutable and cannot shadow visible names or escape their block.
- Resources/mutations/tasks are root-owned. A child may own ordinary state,
  derives, and actions. An action writes only its own admitted slots.
- A `then` handler takes no parameters and must not send its own mutation.
- A state's initializer reads only props, injects and the states above it; a
  resource, derive, mutation, action or later state it names is
  `type-initializer-scope` (LLP 1088 D4).
- An action sends one mutation at most once on any path: a second send forgets
  the first's reply (LLP 1016 D5), so it is `analyze-send-twice` — unless the
  mutation is `queue`, whose sends each wait their turn (LLP 1092 D6). Exclusive
  `if`/`match` arms, and sequential `if`s testing one unchanged name against
  different literals, are separate paths (LLP 1088 D8). The walk reads the root's
  actions with every call expanded, where a caller and its callees are one commit.
- Behind a call, a read of a state, mutation or router slot that another frame
  (the action's own body, or another call) assigned earlier on the same path is
  `analyze-call-stale-read` (LLP 1089 D3): the reader would see the starting
  value, and neither body shows it. Pass the value as an argument, from a `let`
  bound before the write for the old value or the assigned expression for the
  new one. A derive, a resource and `pending(m)` read settled values and are not
  refused; the same walk's exclusive paths are separate.
  `pending`/`failed` operate on declarations, not arbitrary values.
- View roots cannot be conditional/repeated regions. Tags, attributes, and
  children must fit their lowering rules. Class application is not a CSS cascade.
- Scrollers need bounds; virtualized lists have specific axis, template, and
  nesting requirements. The bake additionally checks measured geometry.
- One root route table supplies implicit router state and types. `path` needs a
  literal known route name and the right number of parameter values.
- Source imports stay inside the app directory. A `.contract` import does not
  import arbitrary executable code.
- Timer intervals, literal-only styles, keyframe constants, font files,
  localization tables, native schemas, and optional capabilities have separate
  checks. Runtime/host coverage is not implied by successful parsing.

The [compiler corpus](../contract/corpus) and
[compiler integration tests](../contract/cli/tests/it) contain executable accept
and refusal examples. Use their diagnostic ids to find the rule that rejected a
program rather than expanding this grammar speculatively.
