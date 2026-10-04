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
- Number tokens begin with a digit and contain decimal digits and dots; numeric
  parsing rejects invalid spellings. Use `12` and `12.5`. Negative values are
  unary minus applied to a number. Exponents, separators, and leading-dot
  numbers are not supported forms. Units belong in quoted strings.
- Strings use `"…"`. Escapes accepted by the ordinary string lexer are `\n`,
  `\t`, `\"`, `\\`, ``\` ``, and `\$`. Single quotes are not delimiters.
- Templates use backticks and `${expression}`. Interpolation scanning balances
  nested braces, strings, and templates. A template is one physical source line.
  Template text is verbatim: a backslash is never an escape and stays in the
  text, and `\${` still interpolates. Use an ordinary string when you need an
  escape.
- `//` starts a line comment outside a string/template. No block comments.
- Indentation uses spaces; a tab in indentation is refused. At bracket depth zero, indentation emits
  block tokens. Inside `()`, `[]`, and `{}`, newlines and indentation continue
  the logical expression instead.
- No statement semicolons, single-quoted literals, JSX, or arbitrary braces for
  value interpolation. `{}` belongs to inline option-match syntax, not objects.

The lexer reserves no separate keyword token kind. The parser treats particular
names as keywords at particular sites. See
[`is_keyword` / `is_name_word`](../contract/syntax/src/parser.rs) before relying on
a keyword as an identifier. Shape fields and attribute names deliberately have
more permissive naming contexts.

## Declarations

```ebnf
file          = { declaration } ;
declaration   = use | shape | function | style | keyframes | font
              | routes | component | test ;
use           = "use" IDENT "from" STRING NL ;
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

A `use` path begins with `./`, stays below its importing file without `..`
segments, and resolves to a `.contract` file inside the app directory. Imports
name a component, shape, function, or style. The loader merges the referenced
file's resolved declarations, not just the named declaration; there is no import
namespace. Font declarations and loaded files' keyframes come along with loading. A used file
cannot declare routes. The root file's first component remains the root after
imports are resolved. Duplicate conflicting declarations and cycles are refused.

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
mutation      = "mutation" IDENT "as" "shape" type
                [ "refreshes" IDENT { "," IDENT } ]
                [ "then" IDENT ] NL ;
action        = "action" IDENT [ "(" [ action-params ] ")" ] block(statement) ;
action-params = action-param { "," action-param } [ "," ] ;
action-param  = IDENT [ ":" type ] ;
task          = "task" IDENT "mount" block(schedule) ;
schedule      = "every" "(" ( expr | "frame" ) "," IDENT ")" NL
              | "after" "(" expr "," IDENT ")" NL ;
statement     = IDENT "=" expr NL
              | "let" IDENT "=" expr NL
              | "send" IDENT "=" source-call NL
              | "refresh" IDENT NL
              | IDENT "(" [ arguments ] ")" NL
              | "if" expr block(statement) [ "else" block(statement) ]
              | statement-match ;
statement-match = "match" expr block(statement-case) ;
statement-case  = "case" "some" "(" IDENT ")" block(statement)
                | "case" "none" block(statement) ;
source-call   = IDENT "(" [ arguments ] ")" ;
```

There is one `props`, `inject`, `provide`, `slot`, and `view` section at most in
a component. Only the root may own resources, mutations, and tasks. A task's
body has exactly one schedule. The named timer action takes no parameters;
a millisecond interval is a literal whole number of at least 1.

Both option-match arms are required exactly once. Actions have no loops, returns,
awaits, or ordinary action-to-action calls. A standalone call statement is a
host command, not an arbitrary function invocation. `let` is recognized as the
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
both are present. Action effects are inferred: `writes` clauses are refused.
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
when          = "when" expr block(node) [ "else" block(node) ] ;
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
              | "some" "(" expr ")" | "[" "]" | IDENT
              | IDENT "(" [ arguments ] ")"
              | "(" expr ")" | inline-match ;
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
positional base followed by zero or more replacements. A list literal can only
be empty. There is no object literal, method call, bracket indexing, assignment
expression, `??`, `?.`, or `===`. `any` in an internal roster signature describes
special typing; it is not a source-language type annotation.

## Authored tests

```ebnf
test          = "test" STRING block(step) ;
step          = "tap" STRING [ "hover" ] NL
              | "type" STRING ( STRING | "key" STRING ) NL
              | "clock" ( "settle" | [ "+" ] NUMBER ) NL
              | "screenshot" STRING NL
              | "expect" "tree" ( "has" | "missing" ) STRING NL
              | "expect" "text" STRING "==" STRING NL
              | "expect" "state" IDENT "==" test-value NL ;
test-value    = NUMBER | STRING | "true" | "false" | "none" | "[" "]" ;
```

Targets are driver test ids. `expect state` is deliberately restricted to the
parser's literal cases, not arbitrary expressions or record comparisons. The
parser currently treats unary minus as an expression rather than a number
literal in this particular form. Use the interactive state inspection when a
value lies outside the assertion language. The test compiler emits steps as JSON;
the agent driver executes them. The nine-operation interactive API is larger
than this test-file grammar.

## Standard functions and intrinsics

The complete current `stdlib` name inventory is below. `T` and `U` express generic
relationships enforced by the type checker, not user-declarable type parameters.
Signatures are authored forms; localization's internal lowered signature differs.

| Call | Result / restriction |
| --- | --- |
| `now()` | Milliseconds on the runner's clock since boot (the driver's clock under the agent), not a date: the date is `exactTime().epochAtZero + now()`. A read does not schedule a render |
| `formatTime(ms, offsetMinutes, "short")` | String; fixed offset east of UTC, en-US formatting |
| `formatDate(ms, offsetMinutes, "medium" or "month-year")` | String; format is a literal choice, not an expression containing `or` |
| `formatNumber(n, "compact")` | String; admitted deterministic compact format |
| `length(value)` | Number; list item count or string UTF-16 code-unit count |
| `isEmpty(value)` | Boolean; empty string or list |
| `toString(value)` | String; number, boolean, or string conversion |
| `floor(n)` | Number |
| `max(a, b)` | Number |
| `min(a, b)` | Number |
| `includes(text, substring)` | Boolean, case-sensitive literal substring |
| `startsWith(text, prefix)` | Boolean, case-sensitive |
| `endsWith(text, suffix)` | Boolean, case-sensitive |
| `trim(text)` | String, JavaScript whitespace/line-terminator trimming |
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
| `params(router, parameterName)` | `list<string>` |
| `searchParam(entry, name)` | String |
| `t("key", name=value, …)` | Localized string; validates tables/placeholders |
| `frame(id)` | `Geometry`, actions only; last layout in root space |
| `measure("id")` | `Geometry`, actions only; literal id, height-auto measurement |

The router functions (`open` through `searchParam`) and `encodeRouteSegment`
exist only in an app with a `routes` declaration.

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
| Structure | `main`, `header`, `nav`, `section`, `footer`, `article`, `aside`, `dialog` |
| Text and controls | `text`, `button`, `link`, `input`, `textarea`, `select`, `option` |
| Media / metadata | `image`, `video`, `iframe`, `canvas`, `head` |
| SVG scene | `svg`, `g`, `path`, `polyline`, `polygon`, `circle`, `ellipse`, `line`, `rect` |
| SVG definitions | `defs`, `symbol`, `use`, `clipPath`, `marker`, `mask`, `pattern` |
| SVG color / text / embedding | `linearGradient`, `radialGradient`, `stop`, `tspan`, `foreignObject` |
| SVG filters | `filter`, `feBlend`, `feColorMatrix`, `feComponentTransfer`, `feComposite`, `feConvolveMatrix`, `feDiffuseLighting`, `feDisplacementMap`, `feDropShadow`, `feFlood`, `feFuncR`, `feFuncG`, `feFuncB`, `feFuncA`, `feGaussianBlur`, `feMerge`, `feMergeNode`, `feMorphology`, `feOffset`, `feSpecularLighting`, `feTile`, `feTurbulence`, `feDistantLight`, `fePointLight`, `feSpotLight` |

`text` inside SVG has SVG semantics. A hyphenated tag names a native module: the
bake checks it against `app.json`'s `modules` list (`bake-unknown-module`), and
its attributes pass to the module unchecked. A capitalized name is a component use, not a
built-in tag. Platform support can further restrict an admitted tag, notably
native `foreignObject`.

Style and prop names come from [`schema.json`](../kernel/tables/schema.json) and
[`tags.rs`](../contract/lower/src/tags.rs), including shorthands and contextual
restrictions. This document does not duplicate their changing property tables.
`padding`, `margin`, `inset`, `border-width`, `border-style` and `border-color` take
CSS's one to four values (`padding="12px 40px"`: top and bottom 12, sides 40).
CSS hyphens are part of the authored name. `testId` and admitted host-specific
props retain their declared spelling.

`button appearance="auto"` selects a native control; the literal switch is
resolved after class merging. Default/`none` keeps the authored pressable.
Native face content, styles, transitions/keyframes, and enclosing contexts have
an explicit allowlist in [`controls.rs`](../contract/lower/src/controls.rs).
`buttonStyle` is a styleable host-policy prop whose complete vocabulary is the
schema's `buttonStyles` table; default `bordered`. This adds no tag or new
expression grammar. Platform looks and stand-ins are documented in
[LLP 1069.011](../llp/1069.011-native-buttons.rfc.md).

## Events

An event binding is an action reference or partially applied action. Captured
arguments precede the event payload. The table contains all 40 handler names.
Numeric multi-argument payload ordering should be copied from the feature's
working fixture, not inferred from JavaScript's Event interface.

| Payload appended to captured arguments | Handler names |
| --- | --- |
| One string | `change`, `input` (text field, textarea, `select`), `key`, `message`, `error` |
| One boolean | `hover`; `change`, `input` on a checkbox or `switch` |
| One number | `timeupdate`, `durationchange`; `change`, `input` on `type="range"` |
| One `list<Picked>` | `change`, `input` on `type="file"` |
| One `MarkdownSelection` | `select` |
| Two numbers | `scroll`, `pan`, `panrelease`, `heightrelease` |
| A string, then an `option<string>` | `reorderdrop`: the dragged row's key, then the key it lands before (`none` at the end) |
| Four numbers | `transformgeometry` |
| Six numbers | `transformrelease` |
| Special: zero or one location string, no captured args | `navigate` |
| None | `press`, `cancel`, `focus`, `blur`, `submit`, `load`, `contextmenu`, `dblclick`, `swiperight`, `refresh`, `loadedmetadata`, `play`, `playing`, `pause`, `ended`, `waiting`, `seeking`, `seeked`, `ratechange`, `volumechange`, `canplay`, `reachstart`, `reachend` |

`scroll` appends left then top offsets; `panrelease` appends x/y release velocity;
`heightrelease` appends height and velocity. The compiler validates arity and
available payload types; tags and hosts constrain where events make sense.
`navigate` belongs on the first root element, outside any region, which must
also carry `navigationKey` and `navigationBack`. Transform geometry/release
bindings are required as a pair. See
[`handler_arity`](../contract/analyze/src/lib.rs) and the corresponding corpus/tests.

## Host commands

The current command name inventory is:

`blur`, `copyText`, `deliveryActivate`, `deliveryCheck`, `focus`, `format`,
`openURL`, `selectText`, `setScheme`, `showPicker`, `share`, `saveFile`,
`showOpenFilePicker`, `showDirectoryPicker`, `showSaveFilePicker`, `scrollIntoView`.

These appear only as action statements. They are not ordinary value-returning
functions. Some have dedicated compiler checks while others also rely on host
argument validation. Use the working implementation when selecting arguments:

| Operation | Starting point |
| --- | --- |
| `focus(id)`, editor `format` | [Markdown Stress](../apps/markdown-stress/app.contract) |
| `blur()`, `blur(id)` | [Messages](../apps/messages/app.contract), [keyboard-bar corpus](../contract/corpus/keyboard-bar.contract) |
| `selectText(...)` | [Messages Legacy](../apps/messages-legacy/app.contract) |
| `copyText(text)` | [Messages](../apps/messages/app.contract) |
| `openURL(url)` | No Contract fixture; the hosts' dispatch, such as [`host/web/glue.js`](../host/web/glue.js). The JavaScript web target and Linux do not carry it |
| `setScheme(...)` | [Caltrain](../apps/caltrain/app.contract), [Markdown](../apps/markdown/app.contract) |
| `share(...)` | [share corpus](../contract/corpus/share.contract) |
| `showPicker(id)`, export `saveFile(...)` | [picker tests](../contract/cli/tests/it/picker.rs), [Fieldnotes](../apps/fieldnotes/app.contract) |
| `showOpenFilePicker(id[, multiple])` | [file-picker corpus](../contract/corpus/file-pickers.contract) |
| `showDirectoryPicker(id)` | Same corpus |
| `showSaveFilePicker(id, suggestedName)` | Same corpus |
| `scrollIntoView(...)` | [collection tests](../contract/cli/tests/it/collection_into_view.rs) |
| `deliveryCheck`, `deliveryActivate` | [delivery corpus](../contract/corpus/delivery.contract) |

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
