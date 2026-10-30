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
a `for` body, a `case` arm, a test). The bodies of `if`/`else`,
a `keyframes` or `font` block, and a component need at least one entry.

- A name starts with `[A-Za-z_]`, followed by ASCII alphanumerics/underscores or
  a hyphen immediately followed by an ASCII letter. Thus `font-size` is a name,
  `a-b` is a name, and `a - b` is subtraction.
- Number tokens begin with a digit. A dot is part of the number only when a
  digit follows (`12`, `12.5`), so `rows.0.steps` is a list index and then a
  field. An exponent is part of the number only when digits follow its `e` or
  `E` and optional sign (`1e300`, `2.5E-3`, `4e+2`), so `2em` is a number and a
  unit; one past the largest double is refused. Numeric parsing rejects invalid
  spellings. Negative values are unary minus applied to a number; as an
  attribute's whole value (`min=-40`, `step=-0.5`, `margin-top=-8`,
  `min=-1e300`) it is the negative literal, the number it is, as `40` is, and
  a text field's `min`, `max` and `step` take a number of either sign as a
  range's do. Separators, leading-dot numbers, and a trailing dot are not
  supported forms. Units belong in quoted strings.
- Strings use `"…"`. Their escapes are `\n`, `\t`, `\"`, `\\`, ``\` ``, `\$`,
  and a Unicode code point in hex, as JavaScript and Swift write it: `\u{2011}`
  (one to six digits; `\u{1F600}`, never a surrogate pair) or `\u2011` (four).
  Any other is refused, naming these. Single quotes are not delimiters.
- Templates use backticks and `${expression}`. Interpolation scanning balances
  nested braces, strings, and templates. A template is one physical source line.
  Template text takes a string's escapes (`` `${a}\t${b}` `` is a tab), and
  `\${` is literal text, not an interpolation.
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
LLP 1088 D5): `when`, `if`, `else`, `for`, `in`, `match`, `case`, `as`, `fn`,
`and`, `or`, `not`, `true`, `false`, `none`, `some`. A binder is a component's
props, injects and provided names; states, derives, resources, mutations and actions; action and
`fn` parameters and `fn` names; `let`, a `for` item and index, `case some(x)`,
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
declaration   = use | shape | function | style | decoration | atom | keyframes | font
              | sound | theme | routes | component | members-block | test ;
use           = "use" use-name { "," use-name } "from" STRING NL ;
use-name      = IDENT [ "as" IDENT ] ;
shape         = "shape" IDENT block(field) ;
field         = FIELD ":" type NL ;
function      = "fn" IDENT "(" [ typed-params ] ")" ":" type "="
                ( expr NL | NL INDENT expr NL DEDENT ) ;
typed-params  = typed-param { "," typed-param } [ "," ] ;
typed-param   = IDENT ":" type ;
style         = "style" IDENT block(style-line | part) ;
decoration    = "decoration" IDENT ( style-line | block(style-line) ) ;
atom          = "atom" IDENT style-line ;
style-line    = { FIELD "=" style-literal } NL ;
part          = "::" "placeholder" NL block(style-line) ;
style-literal = STRING | NUMBER | "-" NUMBER | "true" | "false" ;
keyframes     = "keyframes" IDENT block(keyframe-line) ;
keyframe-line = selector { "," selector } { FIELD "=" frame-value } NL ;
selector      = "from" | "to" | NUMBER "%" ;
frame-value   = STRING | NUMBER | "-" NUMBER | constant-call ;
constant-call = IDENT "(" [ arguments ] ")" ;
theme         = "theme" block(IDENT "=" STRING NL) ;   (* the root file's; LLP 1095 D4 *)
font          = "font" STRING ( "=" STRING NL | block(font-face) ) ;
font-face     = NUMBER [ "italic" ] "=" STRING NL ;
sound         = "sound" STRING NL ;
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

A part is CSS's pseudo-element as a block: `::placeholder`, then its
declarations indented beneath it, in a `style` (literal) or under an `input` or
`textarea` before its children (where a value may compute, as the element's own
attributes do). It admits `color` alone (CSS `::placeholder { color }`, LLP 1104
D3), its alpha its own; the field's own block wins over its class's, and without
one the placeholder keeps the platform's colour. Another property in it is
`lower-unknown-attr`; on another element `lower-attr-tag`; `::before` and
`::after` are `syntax-part` (Contract has no generated box: write a child), as
is any other part. `placeholder-color` and `-exact-placeholder-color` are
refused with the block to write.

A state part is a block of one state's rows (LLP 1121 D8): `:active`, `:hover`,
`:focus-visible`, `:disabled` or `:checked` alone on its line, then rows indented
beneath it, in a `style` (literal) or under a node before its children (where a
value may compute). One state a block, once at a place, never nested and never
combined (`syntax-state`). `:disabled` and `:checked` (when `checked`,
`aria-checked`, `aria-selected` or `aria-pressed` is true) take
`background-color`, `color`, `border-color` (each side), `opacity`, `box-shadow`
and `-exact-tint-color`, and fold into the node's own rows, `:checked` under
`:disabled`; `:active` takes `background-color`, `color`, `opacity` and
`-exact-press-scale` and lowers to the press paint (LLP 1061 D7: its `opacity` an
absolute target), `:hover` and `:focus-visible` take `background-color`,
`border-color`, `opacity` and `box-shadow`; a host state's row is a literal
(`lower-state-literal`; a token's name is one). Another row is `lower-state-row`;
a part on a native control, a list's row, or (for `:active`, `:hover` and
`:focus-visible`) a run inside a paragraph is `lower-state-target`. An `:active`
row beside the same paint's `-exact-press-` row is `lower-state-press-paint`.
The node's block wins over its class's, row by row.

A length attribute's string is CSS's own text, read by the kernel: a px or
other absolute length, a percentage, a viewport length, `auto`,
`env(safe-area-inset-*)`, `env(viewport-segment-* x y)`,
`env(titlebar-area-x|y|width|height)`, a layout region's
`env(-exact-<region>-inset-*)`, or a `calc()`, `min()`, `max()` or `clamp()` of
these (`clamp(15px, env(safe-area-inset-bottom), 60px)`; LLP 1001 §2). Anything
else is `lower-attr-value`, with the kernel's reason.

The layout regions (LLP 1123 D10) are the platform's own spacing, measured from
the layout viewport, inside the safe area by default: `layout-margins` (UIKit's
16 or 20 points), `readable-content` (a comfortable reading width), and
`safe-area-corners`, `layout-margins-corners` and `readable-content-corners`,
the same kept clear of rounded corners. `padding-left="env(-exact-layout-margins-inset-left)"`
puts a header where a UIKit screen's would be. On a host without them (the web,
Linux, Windows) each is the safe area.

The title-bar area is
[Window Controls Overlay](https://wicg.github.io/window-controls-overlay/)'s: the part
of the title bar beside the window's controls. On the Mac, under `viewport-fit="cover"`,
it is AppKit's title bar less the traffic lights and their margins (`78 0 822×32` in a
900-point window); all zero in full screen and while a toolbar fills the title bar (an
app with routes has one, LLP 1115.002 D2). An app without routes whose manifest's
`display_override` puts `window-controls-overlay` first gets the platform's tall title row,
an empty unified toolbar over the page, whatever its `viewport-fit`, and the area is that
row's beside the traffic lights; `exactViewport().displayMode` is then
`window-controls-overlay`. Chrome gives the same names to the installed app. Everywhere
else it is undefined, as in a browser tab: `env(titlebar-area-height, 0px)` takes its
fallback, and without one the row takes its initial value.

`-exact-fit="shrink"` on a `text` (CSS has no such property; LLP 1116.001 D1.4) draws
a line too wide for its content box at the scale that fits: its whole line under
`white-space="nowrap"`, else its longest word. It shrinks down to half its size, and
a display size (more than twice the root font size) down to the root size; below that
it overflows, as UIKit's `adjustsFontSizeToFitWidth` truncates. The kernel picks the
scale and every host draws at it; `"none"`, the default, never shrinks.

A `sound` names a WAV under the app's `assets/` (LLP 1096 D1): 16-bit integer
or 32-bit float PCM (or `WAVE_FORMAT_EXTENSIBLE` naming one), one or two
channels, 8–96 kHz, at most 10 s. The compiler reads its header and refuses
anything else with the file named and the conversion to run
(`lower-sound-format`, `lower-sound-long`, `lower-sound-path`,
`lower-sound-unreadable`). `sound` is a word only at the start of a top-level
line, so a `state sound` is a slot. Sounds are app-wide, as fonts are.

A `use` specifier is a relative path (`./` or `../`) to a `.contract` file that
stays inside the using file's root — the app directory, or the package it
belongs to (`contract-use-path`); an `exact:` built-in (`exact:motion`, `exact:content`;
`contract-use-builtin`); or a package name, `name[/sub]` or
`@scope/name[/sub]`, found in the nearest `node_modules` above the using file
and mapped through its `package.json` `exports` (a string, or the `contract` or
`default` condition), else `index.contract` (`contract-use-package`). A package
outside the app needs a `name` in its `package.json` (`contract-package-unnamed`):
its components' identities are named by it (LLP 1121 D15). A root file linked
into the app's tree from elsewhere (a symlink, as conform's corpus fixtures are)
that is in no named package is the app's own, named where the link sits (one in
a named package keeps the package's name); a file a `use` reaches is its
target's.
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
`-exact-exit-animation` literal names, and a `-exact-clock(Name)` literal, resolve in the
file that writes them; a name computed at run time is matched as written. Beside
a computed value, a word is renamed when it is the name whatever the value is;
one that is the name for some values and a keyword for others (`${x} linear
1s`), when it is also keyframes another file's of its name renamed, is refused
(`contract-animation-ambiguous`): quote the name (`'linear'`).
A package's export may be its module declaration, `exact.json` (LLP 1133.001
D1): `use getCurrentPosition, GeolocationPosition from "@exact/geolocation"`
brings the module's sources and sends and its shapes. A module's source is read
by a `resource` and its send by a `send`, each `as shape` what it answers
(`contract-module-kind`, `contract-module-answers`), with keyword arguments
only: the module's options (`maximumAge=600000`), each omitted one left to the
platform, and each platform's words as dotted keywords
(`apple.desiredAccuracy="threeKilometers"`), checked against the words that
platform's implementation declares (`contract-module-positional`,
`contract-module-argument`, `call-argument-twice`). The device answers it, never
the build: its placeholder is `else empty()` (`contract-module-placeholder`).
A send that answers nothing (`"answers": null`, LLP 1133.004 D5) is a statement
in an action, as a host command is (`showNotification(title="Done", tag=t)`,
from `@exact/notifications`: [notifications](reference.md#notifications));
a `send` to one is refused (`contract-module-answers`), and so is a statement
calling a send that answers something (`contract-module-kind`). A module's
event (D6) is heard with an attribute on the root component's outermost node,
`column notificationclick=opened`, naming an action of the root that takes a
prefix of the event's arguments (an untyped parameter takes the event's type;
`contract-event-handler`); anywhere else, or a second time, it is refused
(`contract-event-root`). Each event is delivered once, in order, as its own
commit; a host raises one only once the app has drawn its first frame, so one
that starts the app is held until then.
Fonts are app-wide. A used file cannot declare routes. The root file's first
component remains the root. A name both declared and used
(`contract-use-shadows`), one name used from two declarations
(`contract-use-duplicate`), and cycles are refused.

A route pattern is an absolute path with literal or whole `:name` segments.
Indentation determines its parent. The optional fallback is `notfound` without
a path; with none, the compiler writes it and its screen (a navigation root drawing `for e in stack(nav)`). Route policy fields use attribute syntax and are validated by the
[route/document compiler](../contract/lower/src/routes.rs).

## Components and actions

```ebnf
component     = "component" IDENT block(section) ;
section       = "props" block(field)
              | "inject" block(field)
              | "provide" block(provider)
              | "slot" NL
              | "state" IDENT [ ":" type ] "=" expr [ "persist" ] NL
              | "derive" IDENT "=" expr NL
              | resource | mutation | action | task | sequence
              | "members" STRING NL
              | "view" block(node) ;
members-block = "members" IDENT block(member) ;
member        = "state" IDENT [ ":" type ] "=" expr [ "persist" ] NL
              | "derive" IDENT "=" expr NL
              | resource | mutation | action | task | sequence ;
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
sequence      = "sequence" IDENT ( "mount" | gate ) block(play) ;
play          = "play" "(" expr { "," IDENT "=" expr } ")" NL ;
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

A component may write some of its members in other files (LLP 1091.002), so a
large root can be split by feature under the line cap. `members
"./composer.contract"` in its body stands for the members that file declares
under `members Name`, as if they were written at that line: the program, and
the plan, are the one-file program's. The path is a relative `use` path
(`contract-use-path`). A members file holds one `members` block, named for the
component that lists it (`contract-members-owner`), listed by one line
(`contract-members-twice`, `contract-members-unlisted`); the block holds
`state`, `derive`, `resource`, `mutation`, `action`, `task` and `sequence`,
and the once-only sections and `members` lines stay in the component's own
body (`syntax-members-section`). Every file of a component sees all its
members, props and injects; a name declared in two of its files is refused
naming both (`contract-members-duplicate`). Any other name a members file
calls (a shape, a `fn`, a component) comes from its own `use` lines. The
component's rules hold for its members wherever they are written: a child's
members file declares no resource.

There is one `props`, `inject`, `provide`, `slot`, and `view` section at most in
a component. Only the root may own resources, mutations, and tasks, except that
any component may read the device's facts, `exactViewport()` and `exactPage()`
with no arguments (`EmptyState` reads `accessibilityTextSize`): they open no
request, and every instance reads the root's one answer. A task's
body has exactly one schedule. A gated task (`when cond`, LLP 1092) has its
timer only while `cond` holds, armed from the commit that made it true; `key=`
re-arms it from the commit that changed the key (compared as a `for` key),
and `key=` alone is `when true key=…`. The gate is a bool (`type-task-gate`), the
key a string, number or bool (`type-task-key`), and neither reads `performanceNow()`,
directly or through a derive or a `fn` (`analyze-task-gate-clock`). The named timer action takes no parameters;
a millisecond interval is a literal whole number of at least 1 (`after(1, …)` is the web's
`setTimeout(f, 0)`: 0 would be due in the commit that armed it).

A `sequence` (LLP 1125 D2) declares what should play, as a value, and the
runner plays it: the gate is a task's, and its body is exactly one
`play(tracks, bpm=, from=, countIn=, clock=, clockOut=, ended=)`
(`contract-sequence-body`).
`tracks` is a list of a shape whose fields are, in order, `beats: number`
(the track's length, which it loops at), `loops: number` (0 is forever),
`offset: number`, `output: string` and `steps: S[]`; a step's shape `S` has,
in order, `beat: number` (from the track's start, in quarter notes),
`src: string` (a declared sound, or `""`), `gain: number`, `group: string`,
`note: number`, `velocity: number`, `channel: number`, `duration: number`,
`haptic: string` (a `haptic()` kind, or `""`), then optionally
`playbackRate: number` and `pan: number` (`type-sequence-play`, which also
types the options and refuses an unknown or repeated one). `bpm=` is
required unless `clock=` is given; `from=` (default 0) is the beat at the commit that armed it, and
below 0 it is a count-in: `countIn=`'s steps play once over `[from, 0)`, their
beats counted from `from`. `ended=` names a parameterless action run when the
last finite track ends, at that time. The gate, key and arguments read no
`performanceNow()` (`analyze-task-gate-clock`, `analyze-sequence-clock`), and
only the root declares one (`type-child-resource`). The arguments are read
again after every commit that stands: an equal value changes nothing, and a
different one ends the sequence's voices that have not started (`by edit`)
and plays the new value from that commit, a new `bpm` without a jump in
phase. A step whose `note` is not −1, on a track whose `output` names a MIDI
output, is a note-on there (`velocity` 1–127, `channel` 0–15) and a note-off
`duration` beats later (on `output=""`, no output, it goes to no device and
`exactMIDI().sent` lists it with `output: ""`); a track's `offset` (within ±500 ms) moves its
samples', notes' and haptics' times, as a latency is lined up by ear.
`clockOut="<output id>"` sends Timing Clock 24 times a beat, Start before the
first tick at a beat at or after 0, and Stop at the stop or the end
([MIDI](#midi-exactmidi-sendmidi-midimessage)). `clock="<input id>"`
follows that input's MIDI clock instead (LLP 1125 D9): the sequence is
`waiting` until the input sends Start, which puts beat `from` at the next
tick; its tempo comes from the last 48 ticks' timestamps (`bpm=`, default
120, until 13 have come), its position counts ticks, and a hit not yet within
50 ms of being due that a new tempo or phase moves by more than 2 ms is taken
back (`by clock`) and played again. Stop silences it as the gate would and
leaves it `waiting` (reason `clock`); Continue resumes at the Song Position
sent since, else where Stop held it. The input is opened whether or not an
element hears it, and its Start, Continue, Stop and Song Position are still
heard as `midimessage`. A sequence that follows a clock sends none
(`clockOut=` is ignored and journaled), and a changed `clock=` restarts it,
as a changed key does. One
sequence plays at a time; leaving the foreground on iOS stops it (reason
`background`) unless `audio_session` is `playback` with `audio` in
`host.ios.backgroundModes`. `resource t = exactTransport() as shape T` reads
it: `state` (`stopped`, `waiting`, `countin`, `playing`), `reason` (`gate`,
`ended`, `background`, `bpm`, `clock`, or `""`), `bpm`, `anchorAt` and
`anchorBeat` (the beat now is `anchorBeat + (performanceNow() - anchorAt) *
bpm / 60000`) and `clock` (`internal`, or `midi` while following, when the
tempo is rounded to 0.1 and it and the anchor are said again only when they
move by 0.5 bpm or 1/48 of a beat, or the state does); it is answered by a
commit of its own after each change, as an answer's `then` runs.

### Live Activities: `startActivity`, `updateActivity`, `endActivity`

The three reserved mutation sources (LLP 1126.001 WP9) compile on every host.
They run on iOS through an ActivityKit arm loaded at first use; other hosts
refuse the send with `… is not linked into this artifact`.
Declare the entry in `app.json`'s `live_activities`, its state shape and slots
as in `apps/caltrain/activities.contract`. Each send answers a record whose only field is
`id: string`; the compiler refuses other result shapes:

```contract
shape DepartureState
  status: string
  at: number
shape ActivityHandle
  id: string

component App
  mutation activity as shape ActivityHandle
  action follow(state: DepartureState)
    send activity = startActivity("departure", state)
  action delay(id: string, state: DepartureState)
    send activity = updateActivity(id, state, some("Train delayed"), none)
  action finish(id: string, state: DepartureState)
    send activity = endActivity(id, state, "immediate")
  view
    text "Train"
```

`state` is a record, checked against the entry's state shape at runtime.
An update's optional third argument is `option<string>` (the alert's title,
with the default sound); its optional fourth is `option<number>` (the stale
date in Unix milliseconds). Ending takes `"default"`, `"immediate"`, or a
Unix millisecond dismissal date. A string policy must be one of those literals
or an `enum<"default", "immediate">` value; arbitrary strings are refused at compile time. Attributes plus state obey the same 4 KB
bound as the bake. Update and end validate against the generation in which
the activity started. Keep that state shape while its activities are alive; a
new shape cannot update or end an older instance with incompatible state. A
compatibility-changing binary update follows LLP 1126.001 §5 Q6’s device gate.

`resource activities = exactActivities() as shape Activities` reads a record
whose fields are `activities: Activity[]`, `frequentPushesEnabled: bool` and
`linked: bool` (the host packaged its arm).
An activity has `id`, `kind`, `state` (a JSON string), `activityState`
(`pending`, `active`, `stale`, `ended`, `dismissed`) and `token: option<string>`.
Readers may name only the fields they need. Off iOS the list is empty and
frequent pushes are false. A first demand enumerates existing activities,
including after a relaunch without a send; later token, state and content
updates re-answer the source. A token is absent until ActivityKit supplies
it, and exists only for an entry declaring `push: true`. The data module
sends it to its own backend. Push-to-start, remote notification registration
and the `remote-notification` background mode stay refused.

### MIDI: `exactMIDI`, `sendMIDI`, `midimessage`

MIDI is Web MIDI's, by its names (LLP 1125 D6–D8), and needs the grant line
`device.midi <strings key>` on every host (`Permissions-Policy: midi` on the
web); without it `state` is `denied` and a request is refused by name.
`resource m = exactMIDI() as shape M` reads `state` (`prompt` until
`requestMIDIAccess()` asks, then `granted`, `denied`, or `unavailable` where
there is none: WebKit, tvOS, Linux and Windows), `inputs` and `outputs`, each
a list of a port shape of `id` (stable across replug), `name`,
`manufacturer` and `state` (`connected`, `disconnected`; a port seen stays
listed), and `sent`, the last 128 messages that went out, in the order they went, each
`output`, `data` and `timeStamp` (runner ms; no Timing Clock, nothing taken
back) — any other field is refused at the bake (`bake-midi-field`). It
changes by itself as ports come and go, and, when it declares `sent`, as each
message goes. `requestMIDIAccess()` takes no
arguments (`type-request-midi-access`). `sendMIDI(output, data, at=)` sends
`data`, the bytes of one or more whole MIDI 1.0 messages (no running status,
no SysEx; a literal one is checked, `type-send-midi`), to an output by `id`
at `at` on the runner's clock (the commit's time by default, and the past is
now); what is not well-formed, an output not connected, or the 1,025th
message waiting is dropped and journaled (note-offs and Stop never are).
`""` is no output: what is sent there reaches no device, needs no access, and
`sent` lists it with `output: ""` (an activity log with no device, LLP 1125 D6). The
runner keeps the queue: a host hands a message to its output 50 ms before it
is due and holds it until then, so an edit or a stop takes back what is
still held, and a note already handed still gets its note-off. A sequence's
notes and clock go through the same queue.

`midimessage=action` on an element with `midiinputs=["<input id>", …]` hears
each message from those inputs; every mounted element that declares it hears
it, in tree order, each in its own commit. Its action may take a
`MIDIMessageEvent`: `input`, `data` (`number[]`), `timeStamp` (runner
milliseconds) and `beat` (the sequence's beat then, −1 while stopped or
waiting for a clock). Control Change, pitch bend and pressure keep only their
latest value within one delivery, never across a note of the same input and
channel; notes and transport messages are never dropped or reordered; Timing
Clock and Active Sensing are not dispatched (a followed input's ticks go to
its sequence). `onMIDIMessage` and `midiInputs` are refused with
these names.

`else if c` is `else` around one nested `if c`, in an action and in a view
alike: the same tree, so the same plan. A view refuses `when` and `each`
(`syntax-view-keyword`) and `else when` (`syntax-else-keyword`), naming the
edit.

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
order, each after the reply before it and its `then`. The clauses after the shape
may continue onto deeper-indented lines, each starting with its keyword, in the
same order (`mutation edited as shape Jump refreshes page`, then `then followEdit`
indented under it). Action effects are inferred: `writes` clauses are refused.
Component `contract` sections are also refused.

## Views

```ebnf
node          = element | component-use | view-if | for | view-match | children ;
element       = TAG { attribute | expr | special-word } NL [ element-body ] ;
attribute     = FIELD "=" expr | "autofocus" ;
element-body  = INDENT { continuation | element-part } { node } DEDENT ;
continuation  = FIELD "=" expr { attribute } NL ;
element-part  = "::" "placeholder" NL block(continuation) ;
component-use = CAPITALIZED_IDENT "(" [ named-args ] ")" NL [ node-children ] ;
named-args    = FIELD "=" expr { "," FIELD "=" expr } [ "," ] ;
node-children = INDENT node { node } DEDENT ;
view-if       = "if" expr block(node) [ "else" ( view-if | block(node) ) ] ;
for           = "for" IDENT [ "," IDENT ] "in" expr "key" "=" expr block(node) ;
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

A `for` key is a string, number, or boolean, with unique stable values required
for useful row identity. Its optional second binding is the numeric row index.
`if` conditions are boolean and `match` subjects are options. Root regions are
refused: wrap them in an element. `children` requires a declared slot.

## Expressions and types

```ebnf
type          = type-atom { "[" "]" } ;               (* string[] is a list of strings *)
type-atom     = "number" | "string" | "bool" | "unit" | IDENT | "action"
              | "option" "<" type ">" ;
expr          = ternary ;
ternary       = binary [ "?" expr ":" expr ] ;
binary        = unary { binary-op unary } ;
unary         = ( "-" | "not" | "!" ) unary | postfix ;
postfix       = primary { "." FIELD [ "(" [ arguments ] ")" ] } ;
primary       = NUMBER | STRING | TEMPLATE | "true" | "false" | "none"
              | "some" "(" expr ")" | list | IDENT
              | IDENT "(" [ arguments ] ")"
              | "(" expr ")" | inline-match ;
list          = "[" [ expr { "," expr } [ "," ] ] "]" ;
arguments     = argument { "," argument } [ "," ] ;
argument      = expr | arrow | FIELD { "." FIELD } "=" expr ;
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
uses, a native module's sources and sends (the only place a dotted name,
`apple.desiredAccuracy=`, is one), record construction/copy, localization (`t`), placeholders (`empty`),
canvas `surface=` bindings, and specialized host commands (`share`,
`scrollIntoView`). Ordinary function calls are positional. Arrow expressions are admitted
as the callbacks to `map` and `filter`, with at most item and index parameters;
they are not first-class values or event handlers.

An app-declared shape's call constructs a record: either all named fields, or one
positional base followed by zero or more replacements. A `list` literal is
a list of its items, left to right; their types meet as a ternary's arms do (`type-list-item` when they do
not), and a trailing comma is kept. There is no spread (`[...xs, x]` is
refused), object literal, bracket indexing, assignment
expression, `??`, `?.`, or `===`.

A call on a value, `e.f(args)`, is the roster function `f` with `e` as its first
argument, for the functions the table below writes that way: the web's string
and array methods of that name and meaning (`s.trim()`, `xs.filter(x => …)`,
`n.toFixed(2)`). `e.length` is the count of a list or text, or, when `e` is a
record whose shape declares a field named `length`, that field, as in
TypeScript; on a record without one it is refused. The free spelling (`trim(s)`,
`length(xs)`) is refused with the edit (`syntax-method-form`), and so is any
other method (`xs.reduce(…)`: `syntax-method-call`); the rest of the roster is
called as a function (`first(xs)`, `round(n)`).

A list type is `T[]`, as in TypeScript (`Item[]`, `option<number>[]`); `list<T>`
is refused with the edit (`syntax-list-type`). A `state` may declare its type
(`state picked: option<Item> = none`), which is otherwise inferred; a `derive`'s
type is its expression's and takes no annotation. `any` in an internal roster signature describes
special typing; it is not a source-language type annotation.

## Authored tests

```ebnf
test-file     = { launch | test } ;                   (* a top-level launch line: every test's *)
test          = "test" STRING [ "on" HOST { "," HOST } ] block( { launch } { step } ) ;
HOST          = "web" | "macos" | "ios" | "tvos" | "linux" | "windows" | "android" ;
launch        = "size" NUMBER "x" NUMBER NL          (* written 1200x800 *)
              | "epoch" ( STRING | NUMBER | "now" ) NL (* "2026-09-21T12:00:00Z", Unix ms, or now *)
              | "time-zone" STRING NL                (* an IANA zone, "America/New_York" *)
              | "at" STRING NL                       (* the location it opens at, "/item/7#moves" *)
              | "locale" STRING NL                   (* a BCP 47 tag, "fr-FR" *)
              | "seed" NUMBER NL                     (* 0 through 2^32 - 1 *)
              | "fresh" [ STRING ] NL                (* the Snapback dev server emptied first, seeded from the file *)
              | "before" "data" NL                   (* the first step does not wait for data *)
              | "prefer" IDENT IDENT NL              (* CSS media feature and value, before observations *)
              | "fail" "fetch" STRING [ "times" NUMBER ] NL ; (* armed before the first data load *)
step          = "tap" STRING [ "hover" | "dblclick" | "contextmenu"
                  | "pinch" NUMBER [ "at" NUMBER NUMBER ]
                  | "wheel" [ "-" ] NUMBER [ "-" ] NUMBER
                  | "into" STRING
                  | "modifiers" STRING | "mediasession" STRING [ NUMBER ] ] NL
              | "tap" STRING "drag" ( "to" STRING [ "at" NUMBER NUMBER ]
                  | [ "-" ] NUMBER [ "-" ] NUMBER )
                  { ( "press" | "over" | "hold" ) NUMBER
                  | "from" NUMBER NUMBER | "mouse" }
                  [ "during" { STRING } ] NL
              | "type" STRING ( STRING [ "append" ]
                  | "key" STRING [ "down" | "up" | "for" NUMBER ]
                  | "copy" | "cut" | "paste" STRING ) NL
              | "pick" STRING ( STRING { STRING } | "cancel" ) NL
              | "authorize" [ "cancel" ] NL           (* the held sign-in, against the app's server (oauth recipe) *)
              | "clock" ( "settle" | "data" | [ "+" ] NUMBER [ "real" ] ) NL
              | "clock" "until" "state" IDENT { "." ( IDENT | NUMBER ) } "==" scalar "within" NUMBER NL
                                                     (* real time, the clock frozen, until it holds *)
              | "resize" NUMBER "x" NUMBER NL        (* the window, mid-test: 800x600 *)
              | "prefer" IDENT IDENT NL              (* change a media preference now *)
              | "reload" NL
              | "fail" "fetch" STRING [ "times" NUMBER ] NL (* later fetches whose URL starts with it fail *)
              | "pass" "fetch" STRING NL             (* it stops failing *)
              | "request" METHOD STRING [ json ] [ "as" ( IDENT | STRING ) ] [ "expect" NUMBER ] NL
                                                     (* the driver writes through the app's server *)
              | "close" NL                           (* the window's close button *)
              | "back" NL                            (* the platform's Back *)
              | "screenshot" STRING NL
              | "perf" NL block( step { step } )       (* counts its steps' work; no launch line, `clock … real` or `perf` in it *)
              | "expect" "perf" ( "query" IDENT | ( "built" | "evaluated" ) STRING ) ( "==" | "<=" ) NUMBER NL
                                                     (* a count of the last `perf` block *)
              | "expect" "no" "failures" NL           (* nothing failed that the test did not arrange *)
              | "expect" "tree" ( "has" | "missing" ) STRING NL
              | "expect" ( "disabled" | "enabled" ) STRING NL  (* a control's `disabled` (LLP 1116.010 D3) *)
              | "expect" "refusal" STRING NL          (* one observed refusal of the named mutation slot *)
              | "expect" "text" STRING "==" STRING NL
              | "expect" "state" IDENT { "." ( IDENT | NUMBER ) } "==" test-value NL
              | "expect" "sound" ( "has" | "missing" ) STRING
                  [ "at" NUMBER ] [ "gain" NUMBER ] [ "ends" NUMBER ] [ "by" IDENT ] NL
              | "expect" "mediasession" ( IDENT "==" ( STRING | "none" )
                  | ( "has" | "missing" ) STRING ) NL ;
test-value    = [ "-" ] NUMBER | STRING | "true" | "false" | "none" | "[" "]" ;
scalar        = [ "-" ] NUMBER | STRING | "true" | "false" | "null" ;
METHOD        = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" ;
json          = "{" [ STRING ":" json { "," STRING ":" json } ] "}"   (* strict JSON: quoted keys, no *)
              | "[" [ json { "," json } ] "]"                         (* trailing comma; brackets may *)
              | STRING | [ "-" ] NUMBER | "true" | "false" | "null" ; (* span lines *)
```

`on web` runs a test on the hosts it names and skips it on the others, saying so (LLP 1146 D7):
a test of something only one host has, such as the browser's history after a reload.

`perf` runs its indented steps and counts the work they did (LLP 1146 D3), for the `expect
perf` lines after it. `query channelList` is how often the resource's data source was asked;
`built "MemberList"` the views created and virtual rows constructed at that component's plan
sites (a component the plan declares and never built counts 0), else under the view with that
`testId`; `evaluated` the bindings evaluated there. The counts repeat when the steps and the
answers are the test's, on the agent's clock, so `clock … real` is refused in a block and the
block in a run under platform timing. `query` and `built` read the same on every host;
`evaluated` is each runtime's own, so a test that compares it names one host. The development
source map names components; a host without one says so. A drive's `perf <target> during "…"`
shows the same counts while a test is being written.

`expect no failures` fails when, since the test launched or its last `expect no failures`, a
resource was refused while no `fail fetch` fired, a send failed that no `expect refusal` names,
or the runner said a resource was asked again and again (LLP 1146 D4). Without it, a test
still prints those as `unarranged:` notes, and verify counts them as findings.

Properties live beside the scripts in a test file; a file declaring a component
cannot declare a property. They compile to nothing an app runs. `contract verify`
checks them against each script's observations on the runner and Lean.

```
always "the phone is on one of its five screens"
  expect state screen in "lock" "home" "settings" "display" "messages"

always "the counter is a whole number of at least one"
  expect state counter whole
  expect state counter >= 1

always "a probe's result is absent or an answer"
  expect state tsResult in none some

always "once the photo has loaded it stays loaded"
  expect state ready stays true

after "increment" "increment adds one"
  expect state counter == old + 1

after "reset" "reset puts the turns back and counts itself"
  expect state turns == 0
  expect state resets == old + 1

after "rotate" "rotating touches nothing but the turns"
  only writes turns

after "done" "done writes nothing and focuses the button"
  writes none
  command focus "done"

after clock "the probe never touches what the user set"
  only writes tsResult rustResult rustExecutorResult

never refuses "reset"
```

```ebnf
property      = "always" STRING block( { hold } )
              | "after" ( STRING | "clock" ) STRING block( { hold | frame } )
              | "never" "refuses" STRING NL ;
hold          = "expect" "state" IDENT ( "==" ( test-value | "old" [ ( "+" | "-" ) NUMBER ] | "not" "old" )
                                      | "in" { test-value | "none" | "some" }
                                      | ( ">=" | "<=" ) [ "-" ] NUMBER
                                      | "whole"
                                      | "stays" test-value ) NL ;
frame         = "only" "writes" { IDENT } NL          (* no other root slot changes *)
              | "writes" "none" NL
              | "command" IDENT { test-value } NL ;    (* the commit issued this host command *)
```

`old` is admitted only in `after`, `stays` only in `always`, and frame lines only in
`after`. The braces in the EBNF mean repetition, not written braces. A read names a
whole root state or derive by its bare identifier; fields and indices are stage 2.
Before any script runs, verify rejects a derive whose body, dependencies or called
functions reach a resource, mutation status, clock or geometry read. A mutation's
result slot is a state and may be named.

`always` checks every printed configuration, including boot. `stays v` means that a
previous observation equal to `v` must remain `v`. `after "action"` reads the value
before an input and after its following `land`; the input must have only that
successful handler commit, and its landing only `then` and `queue` commits. Other
commits, including a commit omitted from the trace, make the pair counted only.
`after clock` checks clock advances and landings not paired with inputs, over all
their commits. `only writes` compares every root slot, including queue results.
`never refuses` reads the action on each commit line; an action never run is
reported as not exercised. Poisoned configurations are counted, not checked.
Numbers use binary64 arithmetic and IEEE comparisons; other values compare by
their whole printed text. These checks cover observed steps, not every commit
inside a step, and are not proofs.

`contract test <file>` emits an object with `tests` and `properties` arrays. Each
property carries `kind`, `name`, its declaration `line` and parsed `lines`; each
line has its source `line` and an `op` (`expect-state`, `only-writes`, `command`).
Values carry a `kind` and, for numbers, strings and bools, a `value`. The device
runner currently reads the scripts; property checking on a device is a later lane.

`expect refusal "saved"` names a mutation slot, after `clock data` or `clock settle`
lands its send. It asserts one observed refusal since the previous such assertion for
that slot. Verify accepts that observed failure; other refusals remain findings. A refused
send keeps the slot's prior value and skips its `then`; assert the rendered failure code
or message as well.

Targets are driver test ids. `expect disabled "send"` (`enabled`) reads the control's
`disabled`, which every host's tree carries (LLP 1116.010 D3). A `tap` on a disabled control
does nothing, and fails its step on every host, `send is disabled`: say what the app refuses
with `expect disabled`, then `expect enabled` once what enables it has happened. Each test is a session of its own, opened with its
launch lines: `size 1200x800`, `epoch "2026-09-21T12:00:00Z"`, `time-zone
"America/New_York"`, `locale "fr-FR"` and `seed 7` are the driver's `--size`,
`--epoch`, `--time-zone`, `--locale` and `--seed`, so they lead the test's steps,
each once; `at "/item/7#moves"` opens the app at that location, fragment included,
as `--url` with a path does. Written at the top of the file they apply to every test that does not
name its own; either way they override the drive's flags. Media preferences
(`prefer prefers-color-scheme dark`) also lead a test or the file, and may
change between steps. Defaults merge by feature name; duplicate names among
leading lines are errors. A bad name or value fails that test at its line.
The latest values survive `reload` and verify's relaunch. A file whose
assertions depend on the date says so in the file. `epoch now` is the machine's
clock, read once as each test launches, for a test whose app talks to a live
backend on real time (`snapback4 dev`); its dates differ run to run, so its
expects do not name them. `fresh` empties each `snapback4 dev` server the app
fetches (a loopback `net.fetch` grant) before the test opens, through the
server's development door `POST /dev/reset`; `fresh "seed.json"` then seeds it
with that file's `[{"table": …, "row": {…}}]` (a path from the test file). At
the top of the file it runs before every test. A server without the door (a
snapback4 older than the release that adds it answers 404) fails the test,
saying so. Before the first step, and
after a `reload`, the driver waits for the app's data as `clock data` does (its
module activated, every request in flight answered and each answer's `then`
landed, the clock unmoved); `before data` skips the wait. `fail fetch "<prefix>"`
(LLP 1103) makes every later fetch whose URL starts with the prefix fail as a
refused connection does, on every host: a TypeScript source's `fetch` rejects
with `FetchError` of kind `"Network"`, a Rust source's request settles
`Failed { kind: Network }`, and the request never goes out. `times N` (a
positive whole number) fails only the next N; `pass fetch "<prefix>"` stops it.
Leading the steps, or at the top of the file, `fail fetch` is a launch line,
armed before the app's first data load; later it is a step. Several prefixes may
be armed; the longest matching prefix decides, and arming a prefix again
replaces it. A file's line applies to every test that does not arm the same
prefix itself. A counted fault that matched no fetch by the test's end (or
before it is armed again) fails the test at the line that armed it, and
`reload` relaunches with the table as it is then. A stream (`exactStream`, a
WebSocket) is not matched. `request POST "/polls/1/votes" {"option": 2} as bob`
(LLP 1116.006 D8) is the driver, not the app, writing through the app's server as
another person: a method, a path in quotes under the server URL app.ts calls (its
first local one, `http://127.0.0.1:8787`; or a whole `http://` URL), an optional JSON
body (sent as `content-type: application/json`), and `as <persona>`, sent as
`Authorization: Bearer <persona>` (the `rest` pattern's tokens are `ada` and `grace`;
`as "token-grace"` for a token that is not one word). A status outside 200 through
299, or no answer, fails the step naming the status and the body's start; `expect 409`
after the persona (a status from 100 through 599, LLP 1116.010 D3) is the test of a
refusal: that status passes it, any other (a success too) fails it. The app
shows the write when it next reads, so follow it with `clock +<its poll's ms>` and
`clock data` before the `expect` (a failed `expect` before the clock moved says so).
`test`, `verify` and `agent --test` run the server on an empty database (one already
answering there, your dev server, is refused). Runs take the server's port in turn (another checkout's
run on it is waited for, and named). `fail fetch` does not reach a `request`: the driver sends
it, not the app. Arguments after the entry in app.json's `commands.server` are passed
through in order. For a server that keeps no database, set `"server": { "database": false }`
beside `commands` to omit `DB` and skip scratch database creation and removal.
`tap "id" drag to "other" [at x y]` ends on the other
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
`tap "id" wheel <dx> <dy>` is the driver's wheel (the agent's `tap <t> wheel
dx dy`) at the node's middle, in points (CSS pixels on the web): a positive `dy`
scrolls on toward the content's end, so its offset grows, and a negative one back
toward its start; a positive `dx` scrolls right. It lands at once, with no
momentum, so a Mac's scroll can be tested where a drag selects text instead.
`tap "id" dblclick` and `contextmenu` are the driver's forms of the same names
(no Linux carrier double-clicks). `tap "list" into "key"` brings a virtualized
list's row into view by its key, so a row outside the rendered window can be
tapped by its own id on the next step. `type "id" "text"` sets the field's
value, as Playwright's `fill`; `append` adds the text after the value the tree
shows (a prefilled reply). `type "id" key "Name"` presses the key;
`down` and `up` are the two halves, and `for <ms>` holds it that long on the
virtual clock (`down` or `up` together with `for` is refused). `reload` restarts the app on the store it had: the
web page loads again in the same profile, a native app relaunches on the same
scratch store. Its state starts over and `performanceNow()` is 0 again, at the date the test had
reached (`epoch` plus its `clock` steps); what the app
stored is what it reads, so persistence is testable. `close` presses the
window's close button as ⌘W or the red button does (the driver's `close`): its
`beforeunload` handlers hear it, and a window one keeps stays open, so the
test goes on to the app's own "Save changes?" (on the web the browser's "Leave
site?", answered "Stay"). A window that closes, by `close` or by a press the app
answers with `close()`, takes the session with it, and a step after it fails
naming the line. macOS and the web; iOS and Linux close no window and refuse it.
`back` is the platform's Back, as a person takes it, from what is on top: an
open dialog's cancel (a modal `dialog` or `role="alertdialog"` popover: Escape
on the web and macOS, the alert's Cancel on iOS), else the top sheet dismissed
(a `navigationPresentation="modal"` route: iOS's swipe down), else the pushed
route popped (iOS's Back button, the browser's Back, ⌘[ on macOS, Alt+Left on
Linux). A route goes back as those do: its Back control pressed, else the root's
`navigate` with the location beneath, else the router's own `back`. One the app
refuses (a disabled Back control, `closedby="none"`) is a note, and the test
goes on; with nothing to go back from the step fails. It names no target: `tap`
the app's own control to press it.
`type` on a `select` chooses an enabled option by value, else by its one label;
on a date, time or range input it sets the value in HTML's format; on a checkbox
it takes `true` or `false`, on a radio `true`. A target out of view is scrolled into view first.
An input step ends with what it settled (an answer given in the input's turn,
and its mutation's `then`; on a native host, not one given while another
answer's storage is in flight, [LLP 1097](../llp/1097-storage-that-finishes-after-the-answer.rfc.md)); otherwise the clock stands still between steps:
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
`expect wakelock held` and `expect wakelock released` compare `state.wakelock.held`.
This is the runner's holder record: displayed elements requesting the lock, not
whether the OS or browser granted it. A failure names the holding view ids and
their testIds when present, or says nothing holds it.
`expect sound has "assets/x.wav"`
passes when the runner's record of voices (every host's, under the driver's
clock) holds one of that source matching each clause given: its start (`at`,
runner milliseconds), its `gain`, its end (`ends`), its `playbackRate` and `pan`
(a voice that set neither has 1 and 0), and how it ended (`by
end|group|cut|stop|cancelled`); `missing` is the negation. A dropped call is not
a voice, and a test that asks about a voice the 1,024-voice record no longer
holds fails and says so. MIDI's steps (LLP 1125 §4): `prefer midi.state
granted midi.outputs "OP-XY, IAC" midi.inputs "keys"` sets what a request
finds under the driver (granted unless named) and the ports, by name (a port
named no longer is disconnected); `tap midi "keys" 90 3c 64` is one delivery
from that input now, refused unless the input is connected and a mounted
element hears it or a sequence follows it; `tap midi "keys" clock 125 2000`
sends Timing Clock from that input at that tempo for that long from now, as
an arm does, a delivery every 50 ms with the clock moved to each (a tempo
whose tick is a whole number of milliseconds, such as 125 or 100, keeps
every stamp exact); `expect midi sent "OP-XY" 99 4c 7f at 500` passes when the
runner queued that message for that output and has not taken it back (at
that time, when given), `missing` the negation. Each byte is two hex digits.
`tap "audio" mediasession "seekforward"` calls the
handler the platform would call for that media session action ([LLP
1098](../llp/1098-the-media-session.rfc.md) D10): `play`, `pause` or one of the
six; its number is a seek's `seekOffset` (absent: the element's own) and
`seekto`'s time (required there). It is never a press, needs no box, and is
refused when the element does not own the session or does not offer the action.
`expect mediasession` reads `state.mediaSession`: `owner` (by testId, or
`none`), `title`, `artist`, `album`, `artwork` (as authored) or `playbackState`
`==` a string, or `has`/`missing` an offered action. The test compiler emits steps as JSON;
the agent driver executes them. The nine-operation interactive API is larger
than this test-file grammar.

## Standard functions and intrinsics

The complete current `stdlib` name inventory is below. `T` and `U` express generic
relationships enforced by the type checker, not user-declarable type parameters.
Signatures are authored forms; localization's internal lowered signature differs.
An app's `fn` of one of these names shadows it in every free call (`indexOf(xs, x)`), as
a JavaScript function declared over a global does: a name the inventory gains later
never breaks an app that declared it first. A method call (`xs.indexOf(x)`, `s.length`)
is always the built-in, so a file that declares a `fn`, shape or action of that name and
also calls it as a method is refused (`syntax-method-shadowed`), since the call would
otherwise reach the app's function, which TypeScript's method never does.

`contract vocab <name>` prints one function's entry from these tables (its rows and
examples) beside the roster's signature, so what a row says here is what `vocab`
says. Every roster function has a row and an example, and each example written
`` `call` is `value` `` holds: a contract test evaluates them. A row written
`x.f(…)` is called on its first argument, as the web's method is
([expressions](#expressions-and-types)); the others are called as functions.

| Call | Result / restriction | Example |
| --- | --- | --- |
| `performanceNow()` | Milliseconds on the runner's clock since this launch of the app (it restarts at 0 on every launch; the driver's clock under the agent, from 0), as the web's `performance.now()`, not `Date.now()`: the date is `time.epochAtZero + performanceNow()`, with `resource time = exactTime() as shape T` declared, and that sum is what anything stored or sent takes ([timer recipe](recipes/timer-that-survives-relaunch.md)). `now()` is refused (`type-now-renamed`, LLP 1109 D1) with both repairs. A read does not schedule a render, and a derive that reads it is not read again as time passes (when it is depends on the host's clock), so a value that follows the clock comes from a timer: keep the time in state that a `task … every` action writes | `performanceNow() - startedAt`, in a timer's action, is the milliseconds since `startedAt` |
| `formatTime(ms, offsetMinutes, "short")` | String; fixed offset east of UTC, en-US formatting (`exactTime().utcOffset` is the zone's offset now, answered again when it changes) | `formatTime(1790000000000, 0, "short")` is `"2:13 PM"` |
| `formatDate(ms, offsetMinutes, "medium" or "month-year" or "iso")` | String; format is a literal choice, not an expression containing `or`. `"iso"` is `YYYY-MM-DD`: the date at that wall time, which is `toISOString`'s date part at a whole-minute offset (a fractional offset's sub-millisecond wall time is not clipped again, as no style's is) (LLP 1102 §3.4); every style prints `""` outside years 1–9999 | `formatDate(1790000000000, 0, "medium")` is `"Sep 21, 2026"` |
| `formatDay(then, now, offsetMinutes, "day" or "day-time" or "list")` | String; `then`'s day named from `now`'s, both at the one offset (its calendar days, so a DST change between them is not applied), en-US (LLP 1116.020 D8). `"day"` is `Today`, `Yesterday`, `Tomorrow`, else `formatDate`'s `"medium"`; `"day-time"` adds ` at ` and `formatTime`'s time (`Today at 4:42 PM`, `Oct 11, 2026 at 4:42 PM`); `"list"` is Mail's and Notes' list label: the time today, `Yesterday`, the weekday two to six days back, else `10/11/26` (a later day too). Pure: `now` is the app's ticked clock ([dates recipe](recipes/dates-and-days.md)); `""` when either instant is outside years 1–9999 | `formatDay(1790000000000, 1790000000000 + 86400000, 0, "day")` is `"Yesterday"` |
| `formatNumber(n, "compact")` | String; admitted deterministic compact format (`1.2K`) | `formatNumber(1249, "compact")` is `"1.2K"` |
| `formatNumber(n, "decimal")`, `formatNumber(n, "percent")`, `formatNumber(n, "currency", "USD")` | String; `Intl.NumberFormat("en-US")`'s deterministic forms (LLP 1116 D8): `"decimal"` is grouped with at most three fraction digits (`1,234.567`), `"percent"` multiplies by 100 with none (`0.256` is `26%`), `"currency"` takes an ISO 4217 code literal and prints its symbol and fraction digits (`$1,481.47`, `-$5.00`, `¥1,235`, `CHF 5.00`; the codes are USD, EUR, GBP, JPY, CNY, INR, CAD, AUD, NZD, HKD, SGD, CHF, SEK, NOK, DKK, PLN, MXN, BRL, KRW, ZAR, TWD, ILS, PHP and VND). Each rounds the value's shortest decimal digits half away from zero, as `Intl` does (`1.005` is `$1.01`, where `toFixed` says `1.00`), and signs a negative zero (`-0`); `NaN` and the infinities print `""`. The code is written with `"currency"` only (`type-format-style`) | `formatNumber(1481.47, "currency", "USD")` is `"$1,481.47"` |
| `n.toFixed(digits)` | String; JavaScript's `Number.prototype.toFixed`: the binary value rounded (`(1.005).toFixed(2)` is `"1.00"`), a tie away from zero, `-0.001` at 2 is `"-0.00"`, `1e21` and up as `toString` prints; one declared difference: `NaN` and the infinities print `""` (LLP 1054.000.003 D7). `digits` is a whole-number literal 0–100 (LLP 1102 §3.2) | `(3.14159).toFixed(2)` is `"3.14"` |
| `formatDecimal(units, digits)` | String; an integer count of a smallest unit as a decimal, exactly: `formatDecimal(1234, 2)` is `"12.34"`, `formatDecimal(-5, 2)` is `"-0.05"`, `-0` is `"0.00"`; a count that is not an integer, or not finite, is `""`, so money is a count of cents, or `formatDecimal(round(price * 100), 2)` for a price of at most two decimals under a trillion. `digits` is a whole-number literal 0–20 (LLP 1102 §3.2) | `formatDecimal(1234, 2)` is `"12.34"` |
| `value.length` | Number; list item count or string UTF-16 code-unit count | `[4, 5, 6].length` is `3` |
| `value.isEmpty()` | Boolean; empty string or list | `"".isEmpty()` is `true` |
| `value.toString()` | String; number, boolean, or string conversion | `(42).toString()` is `"42"` |
| `floor(n)` | Number | `floor(2.7)` is `2` |
| `ceil(n)` | Number; `Math.ceil` | `ceil(2.1)` is `3` |
| `round(n)` | Number; JavaScript's `Math.round`: a half rounds up (`round(2.5)` is 3, `round(-2.5)` is -2), not away from zero. Two decimals is `round(v * 100) / 100` | `round(2.5)` is `3` |
| `parseNumber(text)` | `option<number>`; `some` for a decimal numeral in the text, trimmed as `trim` does: an optional sign, digits with an optional fraction or a fraction alone, an optional exponent (`" 12.5 "`, `"-3"`, `".5"`, `"1e3"`), the nearest double as `Number()` reads it; `none` for anything else (`""`, `"12px"`, `"0x1F"`, `"1_000"`, `"Infinity"`), past the largest finite, or a nonzero numeral that rounds to zero (LLP 1102 §3.1) | `parseNumber(" 12.5 ")` is `some(12.5)` |
| `calendarDiff(from, to, "years" or "months")` | `option<number>`; whole years or months from one `YYYY-MM-DD` date to another, counted as an age is: a period completes when `to`'s month and day reach `from`'s (months compare the day), so a Feb 29 start completes a year on Mar 1 of a common year and a Jan 31 start a month on Mar 1, as Temporal's `PlainDate.until` counts with `largestUnit` `"years"` or `"months"`. When `to` is earlier, the count back, negated; `none` when either is not a real date (LLP 1102 §3.4) | `calendarDiff("1990-05-10", "2026-05-09", "years")` is `some(35)` |
| `max(a, b)` | Number | `max(3, 7)` is `7` |
| `min(a, b)` | Number | `min(3, 7)` is `3` |
| `sin(rad)` | Number; `Math.sin`, CSS's `sin()`. The native runners' `libm` gives the same bits on every target; the web's `Math.sin` may differ in the last bits | `sin(0)` is `0` |
| `cos(rad)` | Number; `Math.cos`, CSS's `cos()`, computed as `sin` is (a point on a ring is `cx + r * cos(a)`, `cy + r * sin(a)`) | `cos(0)` is `1` |
| `sum(xs)` | Number; a list of numbers added left to right from 0, as `xs.reduce((a, x) => a + x, 0)` (`sum([])` is `0`), a list step an item; `sum(xs.map(x => …))` totals a field | `sum([1, 2, 3])` is `6` |
| `range(n)` | `number[]`; Exact's own, not the web's: the whole numbers from 0 to `n - 1` in order (`range(0)` is `[]`), a list step an item. `n` is a whole number at least 0: another literal is refused, and another computed value traps (`TypeMismatch`), refusing the commit that made it. `range(a, b)` is refused: the numbers from `a` are `range(b).slice(a)`, a window `range(n).slice(a, b)` | `range(4)` is `[0, 1, 2, 3]` |
| `text.includes(substring)` | Boolean, case-sensitive literal substring | `"grocery".includes("roc")` is `true` |
| `xs.includes(item)` | Boolean; `T` a string, number or bool, compared by SameValueZero (`NaN` is found, `-0` is `0`) | `["a", "b"].includes("b")` is `true` |
| `text.startsWith(prefix)` | Boolean, case-sensitive | `"/settings".startsWith("/")` is `true` |
| `text.endsWith(suffix)` | Boolean, case-sensitive | `"photo.png".endsWith(".png")` is `true` |
| `text.trim()` | String, JavaScript whitespace/line-terminator trimming | `"  milk ".trim()` is `"milk"` |
| `text.slice(start, end?)` | String; JavaScript's `slice` over UTF-16 code units: fractions truncate, NaN is 0, a negative index counts from the end, an omitted `end` is the end; a cut surrogate half is U+FFFD | `"Tuesday".slice(0, 3)` is `"Tue"` |
| `xs.slice(start, end?)` | `T[]`; the items, the indices clamped as text's are | `[1, 2, 3, 4].slice(1)` is `[2, 3, 4]` |
| `xs.concat(T[])` | `T[]`; both lists' items in order (`[...xs, x]` is `xs.concat([x])`) | `[1, 2].concat([3])` is `[1, 2, 3]` |
| `xs.flat()` | `T[]`; a list of lists' items in order, one level, as the web's `flat()`. It takes no depth (`flat(2)` is refused): two levels are `xs.flat().flat()`, and the web's `flatMap` is `xs.map(x => …).flat()` when the callback returns a list (`xs.map(x => …)` when it returns one value) | `[[1], [2, 3]].flat()` is `[1, 2, 3]` |
| `text.indexOf(substring)` | Number; the first match's position in UTF-16 code units, `-1` for none, `0` for `""` | `"a,b".indexOf(",")` is `1` |
| `xs.indexOf(item)` | Number; `T` a string, number or bool, compared by `===` (`NaN` is never found, `-0` is `0`); `-1` for none | `["a", "b"].indexOf("c")` is `-1` |
| `text.split(separator)` | `string[]`; the pieces between the separator's matches, left to right; `""` splits into UTF-16 code units (a cut surrogate half is U+FFFD), and `"".split("")` is `[]` | `"a,b,c".split(",")` is `["a", "b", "c"]` |
| `text.replaceAll(find, with)` | String; every match of the string `find`, left to right; `$$`, `$&`, `` $` ``, `$'` in `with` (`$1` is literal); an empty `find` inserts at every code-unit boundary; no regular expressions | `"a-b-c".replaceAll("-", " ")` is `"a b c"` |
| `text.toLowerCase()` | String; Unicode's default lowercase, final sigma kept, no locale (the web core links the case tables by use) | `"Milk".toLowerCase()` is `"milk"` |
| `text.toUpperCase()` | String; Unicode's default uppercase through the same tables, its full mappings included (`"ß"` is `"SS"`), no locale. Text on screen is uppercased by `text-transform="uppercase"`, which each platform applies as it draws; `toUpperCase` is for a string handed elsewhere: a native view's argument, `share`, a value stored or sent | `"Straße".toUpperCase()` is `"STRASSE"` |
| `first(T[])` | `option<T>` | `first([4, 5])` is `some(4)` |
| `xs.at(index)` | `option<T>`; truncates index toward zero, negative from end | `[4, 5, 6].at(-1)` is `some(6)` |
| `xs.map(callback)` | `U[]`; callback returns one value | `[1, 2].map(x => x * 10)` is `[10, 20]` |
| `xs.filter(callback)` | `T[]`; callback returns boolean | `[1, 2, 3].filter(x => x > 1)` is `[2, 3]` |
| `xs.join(separator)` | String; `xs` a list of strings, numbers or bools, each printed | `["a", "b"].join(", ")` is `"a, b"` |
| `sortBy(xs, x => key)`, `sortBy(xs, x => key, "desc")` | `T[]`; the items in a stable order of one key, a number or a string, computed once an item (the callback is `map`'s, item and index). Numbers ascend by `<` with `NaN` counted as `0` (so `NaN`, `-0` and `0` tie); strings by UTF-16 code units as `<` compares them, no locale. `"desc"` (a literal, as is `"asc"`, the default) reverses the order of the keys, not of ties: equal keys keep their input order in both directions, so a second key is an earlier sort (`sortBy(sortBy(xs, x => x.created), x => x.bpm)` is by bpm, ties oldest first). Exact's own function, not TypeScript's `toSorted`, whose comparator is refused; a locale's collation, a stored order and rows the view has not loaded are the data module's (LLP 1116.016 §5) | `sortBy([3, 1, 2], x => x)` is `[1, 2, 3]` |
| `encodeURIComponent(text)` | String | `encodeURIComponent("a b&c")` is `"a%20b%26c"` |
| `encodeRouteSegment(text)` | String; routing's checked segment encoding | `encodeRouteSegment("a b")` is `"a%20b"` |
| `open(router, location)` | Router | `top(open(nav, "/item/7")).name` is `"item"` |
| `push(router, location)` | Router; same top URL does not add a visit | `depth(push(nav, "/item/7"))` is `2` |
| `replace(router, location)` | Router; the top visit takes the location and keeps its id. A stack's first visit keeps its root route: a location matching another route is refused and the router is unchanged (a first-launch screen: docs/recipes/first-launch.md) | `top(replace(push(nav, "/item/7"), "/item/8")).url` is `"/item/8"` |
| `back(router)` | Router | `depth(back(push(nav, "/item/7")))` is `1` |
| `select(router, tabName)` | Router | `nav = select(nav, "search")`, in an action, shows the `search` tab |
| `go(router, location)` | Router; the top, a pop to it on the selected stack, another tab whose top it is, or else a push onto the selected stack, whatever tab declares its route (into the route's declared tab: `open`) | `top(go(nav, "/item/7")).name` is `"item"` |
| `stack(router)` | `Entry[]` | `stack(push(nav, "/item/7")).length` is `2` |
| `top(router)` | `Entry` | `top(nav).name` is `"home"` |
| `depth(router)` | Number | `depth(nav)` is `1` |
| `params(router, parameterName)` | `string[]`: that parameter's non-empty values over the selected stack, bottom first, duplicates kept (one entry's own value is `entry.params.<name>`, `""` where its route does not bind it; the name must be a parameter of some route) | `params(push(nav, "/item/7"), "id")` is `["7"]` |
| `searchParam(entry, name)` | String | `searchParam(top(push(nav, "/item/7?tab=notes")), "tab")` is `"notes"` |
| `fragment(entry)` | String: what the entry's location had after `#`, fragment-encoded as the browser's `hash` is, `""` when none. It is no part of the screen: `/item/7#moves` is the visit to `/item/7`, and a `push`, `replace` or `go` that changes only the fragment changes it on that visit. Written as `path("item", id) + "#moves"`, or `href="#moves"` on a `link` to the screen shown. Once the route's content has arrived, each host scrolls to the element with that `id` in the entry's route, as a browser scrolls to a fragment (`top`, or no such element: the route's top) (LLP 1153 D5) | `fragment(top(push(nav, "/item/7#moves")))` is `"moves"` |
| `sharedFiles(entry, name)` | `SharedFiles`: `{files: Picked[], dropped, pending}`, the files a received share carried, by the token in the entry's `name` parameter; `pending` counts those still being copied, and a token the host did not set this launch answers no files ([receiving shares](../llp/1133.024-share-browser-compose-print.rfc.md), D3) | `sharedFiles(top(nav), "files").files.length` is the count of shared files that have landed |
| `t("key", name=value, …)` | Localized string; validates tables/placeholders | `t("greeting", name=user.name)` is the `greeting` string of the app's locale with `{name}` filled |
| `frame(id)` | `Geometry`, actions only; last layout where the viewer sees it, as `getBoundingClientRect`: in the viewport, every scroll offset and transform applied | `frame("card").height`, in an action, is the card's height as drawn |
| `rangeFrame(id, start, end)` | `Geometry`, actions only; a textarea's UTF-16 range as `Range.getBoundingClientRect()` (on a `text markup="markdown"`, the union of the atom pills the range meets, [inline atoms](#inline-atoms-atom-atoms-atomfaces-atompress)): collapsed, the caret's rectangle; unavailable past the text, while composing, or with no host reader ([decorations](#plain-textarea-decorations), LLP 1114.007) | `rangeFrame("composer", caret, caret)`, in an action, is where a mention popover goes |
| `measure("id")` | `Geometry`, actions only; literal id, the border box as if `height` were `auto`, other styles kept (padding, border, `min-height`, `max-height`, insets): 0 for a bare box whose children are all absolutely positioned (a navigation root); `frame` is the laid-out box | `measure("note").height`, in an action, is the note's natural height |
| `elementFromPoint(x, y)` | `option<string>`, actions only; the `id` of the front-most of `frame`'s boxes at the viewport point, or of its nearest ancestor with one (DOM's `elementFromPoint(x, y)?.closest("[id]")?.id`): ancestors' overflow clips apply, `pointer-events: none` and hidden boxes are passed over | `elementFromPoint(x, y)`, in a pointer action, is `some("card-3")` over that card |
| `elementsFromPoint(x, y)` | `string[]`, actions only; own ids of every hit box, including hit ancestors, front to back; id-less boxes skipped, with `elementFromPoint`'s clips, transforms and skips | `elementsFromPoint(e.clientX, e.clientY)` sees through an Inspect overlay |
| `querySelectorAll(selector)` | `string[]`, actions only; only `[data-word~="value"]` for a declared `data-*` word; own ids of mounted, displayed, non-inert matches in tree order, excluding covered routes and unselected tabs; any other dynamic selector gives `[]` | `querySelectorAll("[data-paint~=\"accent\"]")` lists the accent nodes |

On Apple, Linux and Windows, `frame` of an inline `text` run returns the
viewport-space union of its line fragments, using the run's own font ascent
and descent and excluding collapsible spaces at a soft wrap. Preserved spaces
and tabs remain in the rectangle, including at a soft wrap. An empty run has a
zero-width rectangle at its insertion point with its font's height. Scrolling
moves this rectangle; clipping does not trim it. Linux and Windows read the acknowledged
paragraph, including multi-column placement and affine transforms; a
perspective-projected paragraph is unavailable. These readers give a cross-run
ligature's advance to its first character's run; later runs have zero-width
rectangles at its end, as in Chrome. Windows shares the Linux presenter;
runtime verification was on Linux. Android and the terminal have no inline-run
reader.

For `elementsFromPoint`, an id-less hit outside a named ancestor's own box
includes that ancestor once before lower layers. This keeps its first entry
equal to `elementFromPoint`'s nearest-id answer; LLP 1001 §1 declares the
rare difference from Chrome's raw filtered list.
An id-less hit inside that box is skipped even when it covers a named sibling.
Both point reads skip covered navigation routes, inert subtrees, and closed dialogs or popovers. Open painted top-layer content uses its presented frame and escapes clips on its authored ancestors. A macOS alert projected as an `NSAlert` sheet or an iOS UIKit confirmation has no authored hit boxes.
For `querySelectorAll`, a closed popover or dialog is hidden by its presenter;
an open one is included when it matches. A wide split list is included only
where the host presents it (on the web); native hosts hide the unselected route.

The router functions (`open` through `sharedFiles`) and `encodeRouteSegment`
exist only in an app with a `routes` declaration.

`<`, `<=`, `>` and `>=` take two numbers or two strings. Two strings compare as
JavaScript's do, by UTF-16 code units in order with a proper prefix first, no
locale (`"09:30" < "10:00"`, `"Z" < "a"`). `slice` and `replaceAll` work on code
units as JavaScript does and make the whole result well formed once: a lone
surrogate half is U+FFFD, since the native runners hold Unicode scalar values.
A result past the runner's string bound (64 MiB of UTF-8) traps on every
executor. `padStart` is not in Contract;
the refusals say what to write instead. Neither `indexOf` nor `split` takes
the web's second argument (a start position, a limit), and `split` takes no
regular expression.

`concat`, `split`, `sum`, `range`, `flat`, and `slice`, `includes` and `indexOf` over a list, take
list steps on the evaluation's budget as `map` and `join` do (LLP 1090): one
for each item `concat`, `slice`, `split`, `range` or `flat` makes (and `flat` one for each list it reads), taken before the list is
built, one for each item `sum` adds, and one for each item `includes` or
`indexOf` scans up to its match. A
list they build is bounded as any built value is. `sortBy` takes one step an
item, as its key is computed, and none for its comparisons: a stable merge sort
makes at most n·⌈log₂ n⌉ of them, which is the cost the budget does not count.
So 5,000 rows filtered and then sorted take 10,000 of the 65,536 steps, and
the sort of 30,000 is about 450,000 comparisons on every evaluation that reads
it, which a `derive` makes once per change of what it reads.

Compiler intrinsics and special forms additionally include:

| Form | Meaning | Example |
| --- | --- | --- |
| `pending(resourceOrMutationName)` | Boolean; request is in flight | `pending(saved)` is `true` from `send saved = …` until its reply |
| `failed(resourceOrMutationName)` | Boolean; current resource request failed without an answer | `failed(weather)` is `true` once the weather request fails |
| `failure(resourceOrMutationName)` | `option<Failure>`: `none` until the current request fails without an answer, then `some({ code, message, reason })` (LLP 1109 D3); `code` is the nonempty source `reason` when supplied, otherwise the infrastructure vocabulary below, the same on every host; `message` is what the agent's `state.failed` shows, and says more and differs by host; `reason` is the source error’s string `reason` (a Snapback refusal’s code such as `SLOT_FULL` or `E_RULE`), else `""`, on every host | `match failure(weather)` with `case some(f)` reads `f.code`, `"offline"` with no connection |
| `path("routeName", args…)` | Checked encoded route location | `path("item", "7")` is `"/item/7"` |
| `empty(field=constant, …)` | Resource placeholder's zero record with overrides | `resource days = loadDays() as shape Days else empty()` shows `Days`' zero until the answer |
| `some(expr)` | Construct an option | `some(3)` is an `option<number>` holding 3 |
| `DeclaredShape(field=value, …)` | Construct a record | `Day(key="mon", ml=0)` is a `Day` |
| `DeclaredShape(base, field=value, …)` | Copy a record | `Day(d, ml=d.ml + 250)` is `d` with `ml` changed |

`failure(x).code` preserves a domain refusal code (`SLOT_FULL`, `E_RULE`, `E_OFFLINE`)
when the source throws an error carrying that string as `reason`. Otherwise it uses
one of six infrastructure words, by what reached the runner
(`runner/src/failure.rs`):

| Code | When |
| --- | --- |
| `offline` | A request the source made reached no server (no connection, DNS, TLS, a reset, the driver's `fail fetch`) and the source let the rejection through |
| `timeout` | A request's deadline (`exactTimeout`) passed, let through |
| `refused` | The host refused a request outside the app's grants or limits (admission, or a response over its size limit), let through |
| `shape` | The answer is outside the resource's declared shape |
| `storage` | A storage call failed (a coded storage refusal: `denied`, `full`, `EBUSY`, a filesystem error, storage unavailable here), let through |
| `error` | Anything else: the source's own error (a TypeScript `throw` or rejection, a Rust source's `Err`), an aborted request, a host that cannot make it, a module over its time budget |

"Let through" means the TypeScript module rethrew the `fetch` or storage
rejection or did not catch it; an error the module makes of its own is
`error`. An HTTP error status is an answer, never a failure: a module that
throws on a 500 fails with `error`. A Rust source's own error is `error` on
every host. A request the caller aborted is `error`. A response over its size
limit (`exactIndependentHttp.maxResponseBytes`, else the host's 64 MiB; a
stream's event or message over its ceiling) is `refused` on every host: the
server answered, and the host will not take more than the app said it would;
asking again gets the same answer, which `offline` would not imply. A
new code is a breaking change; branch with a default case.

`failure(x).reason` retains the domain string as well, empty when none was supplied.
For a mutation, failure belongs to its latest send: a new send, a successful answer,
or an explicit assignment clears it. A failed request retains its previous value,
does not retry, and does not run `then`. A synchronous throw that refuses the sending
action still rolls that action back; only an admitted request's failure persists.

Finite string parameters are written `enum<"draft", "published">`. Members must be
quoted, distinct strings, with at least one member. A literal call argument must be a
member; a dynamic argument must have a compatible declared enum type. Shapes may
use enums too; a source answer outside the declared vocabulary fails its shape check.
Generated TypeScript uses the corresponding string literal union. An enum's placeholder
is its first member. Enums use ordinary string values at the data seam.

`Router`, `Tab`, `Entry`, and `Params` are introduced by routes. The compiler
also declares `Geometry` (geometry reads), `Failure` (`failure(x)`'s
`{ code: string, message: string, reason: string }`), `MarkdownSelection` (the `select`
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
| Tables / description lists | `table`, `caption`, `thead`, `tbody`, `tfoot`, `tr`, `th`, `td`; `dl`, `dt`, `dd` |
| Text and controls | `text`, `h1`–`h6`, `button`, `link`, `input`, `textarea`, `select`, `option`, `progress` |
| Media / metadata | `image`, `video`, `audio`, `iframe`, `canvas`, `head` |
| SVG scene | `svg`, `g`, `path`, `polyline`, `polygon`, `circle`, `ellipse`, `line`, `rect` |
| SVG definitions | `defs`, `symbol`, `use`, `clipPath`, `marker`, `mask`, `pattern` |
| SVG color / text / embedding | `linearGradient`, `radialGradient`, `stop`, `tspan`, `foreignObject` |
| SVG filters | `filter`, `feBlend`, `feColorMatrix`, `feComponentTransfer`, `feComposite`, `feConvolveMatrix`, `feDiffuseLighting`, `feDisplacementMap`, `feDropShadow`, `feFlood`, `feFuncR`, `feFuncG`, `feFuncB`, `feFuncA`, `feGaussianBlur`, `feMerge`, `feMergeNode`, `feMorphology`, `feOffset`, `feSpecularLighting`, `feTile`, `feTurbulence`, `feDistantLight`, `fePointLight`, `feSpotLight` |

`h1` to `h6` are `text role="heading" aria-level=1` to `6` (LLP 1116.017 D1). `text` inside SVG has SVG semantics. A hyphenated tag names a native module: the
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
name, no name for the card (what an app writes, at most 8 KB), `--all` for everything,
`--json` for a document. From an app made by
`exact new`, run `bun exact.mjs contract vocab`.
`padding`, `margin`, `inset`, `border-width`, `border-style` and `border-color` take
CSS's one to four values (`padding="12px 40px"`: top and bottom 12, sides 40), and
`border-radius` its one to four corners (`border-radius="18px 18px 0 0"`: top-left,
top-right, bottom-right, bottom-left; no `/` elliptical radii), as a literal or a
literal arm; `gap` takes its row and column gaps, and `overflow` and
`overscroll-behavior` their x and y values, the same way (`gap="8px 16px"`,
`overflow="hidden auto"`). A template is split the same way at its own text's white space,
each interpolation part of one value that also holds text, outside parentheses
(`` border-radius=`${r}px ${r}px 0 0` ``: the top corners `r`). Another list (`` `${a} ${b}` ``,
a concatenation, a `fn` that returns one) is refused
(`lower-css-shorthand`): write the longhands, each of which may be computed. A computed
single value (`padding=pad`) is every side's.
An empty string on any style row declares nothing, as CSSOM's `setProperty(p, "")`:
the row is unset (`background-color=dot` with `dot` `""`).
`border-style` and each `border-<side>-style` accept `none`, `hidden`, `solid`,
`inset`, `dotted` and `dashed`. The two patterned styles keep their authored
border widths in layout. AppKit, UIKit and Linux paint their dots and dashes;
`column-rule-style` still accepts only `none`, `hidden` and `solid` on native hosts.
CSS hyphens are part of the authored name. `testId` and admitted host-specific
props retain their declared spelling.
`text-wrap-style="balance"` and its `text-wrap="balance"` shorthand are
inherited; `auto` restores ordinary greedy wrapping. `pretty` and `stable` are refused with the supported values, `auto | balance`. Every host balances the
lines before a paragraph's first forced break, up to six, at the narrowest
width that keeps their count, as Chrome does (LLP 1001 §6).
`-webkit-font-smoothing` takes `auto` or `antialiased` and is inherited; `none`
and `subpixel-antialiased` are refused. It changes how glyphs paint, never how
text measures (LLP 1019 D12).

`-exact-hairline` is a length every length row takes but a font's size and line
height ([LLP 1115.000](../llp/1115.000-no-tells-on-every-host.rfc.md) D10.3): one
device pixel, 1 px over the display's scale, which each host tells the kernel as
it tells it the root font size (UIKit's `displayScale`, AppKit's
`backingScaleFactor`, the browser's `devicePixelRatio`, the pixels a point a
Linux, Android or Windows shell draws at; 1 where a host says none). It is a
third of a point on a phone and half on a Retina Mac, in layout and in paint,
and follows a window to another display. Write it where a separator's
thickness was `0.5`, which is one device pixel only at 2x: `border-width`, the
`border` shorthand (`border-bottom="-exact-hairline solid -exact-separator"`), a
`height`, a `gap`; literal or bound. `hr` is that line: one solid top edge a
hairline thick in the separator role, as wide as what holds it (a flex column
included), with no margin of its own. Its `color`, `border-*` and `margin` are
yours to write.

`-exact-accessory-checkmark-width` and `-exact-accessory-checkmark-height` are
length ramps ([LLP 1137](../llp/1137-length-ramps.rfc.md)): UIKit's list-cell
checkmark at the root font size, growing with the text size as a text style
does (19 × 17.33 at the default size, 48 × 43.33 at the largest). They are
taken where `-exact-hairline` is; a grouped list's checkmark in a row of your
own already has them on iOS. `-exact-accessibility-stack-gap` is 8 through the
largest standard size and wider than any line at the accessibility sizes: as
the `column-gap` of a row that wraps, it stacks the row's items there, as
UIKit's cells do (a grouped row's title and value already take it on iOS).

`-exact-row-look="platform"` on an element says its box's look (its
`background`, `border`, `padding` and `box-shadow` rows) is its own where it stands
alone and the platform's row where it is a grouped list's row ([LLP
1139](../llp/1139-a-row-look-that-yields.rfc.md)): there those rows are dropped
and the list draws the row, as Exact Content's `AnswerTile` is a tile in a quiz and a
list's cell in a poll. Elsewhere it does nothing. A grouped row's edge or background
image its author wrote otherwise stays the author's on every host, iOS's carried row
included.

A `button` is the platform's own control by default ([LLP
1104](../llp/1104-the-platforms-controls-by-default.rfc.md) D1). Your bare box
(`appearance="none"`, or a button given a background, border, radius or rich
children) lays out as Chrome's `<button>` does with Exact's reset ([LLP
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
open. An `href` that turns out empty is no link: the run or `link` has no
link colour, role or pointer and follows nothing, on every host, so a
component may write `href=r.href` on every run and leave plain prose's empty.
A relative path (`note/3`) has no base natively; write it from `/`. One to an absolute URL (`https://…`, `//…`) leaves the
app: natively it opens in the system browser (the Linux host has none and
logs it), and on the web in a new
browsing context (`target="_blank" rel="external noopener"`), so the app is
still there when the reader comes back. On the web, `target="_self"` keeps
such a link in the page (it replaces the app) and `target="_blank"` opens a
path in a new one; natively `target` changes nothing, an app having no
other tab. `mailto:` and `tel:` links go to their handlers everywhere.

An unset `appearance` is `auto`: the platform's button, unless a background,
border, radius, rich children or a row the native control cannot carry makes it
your bare box. `appearance="auto"` written out asks for the platform's button and
refuses what it cannot carry; `appearance="none"` asks for your box. The literal
switch is resolved after class merging.
Native face content, styles, transitions/keyframes, and enclosing contexts have
an explicit allowlist in [`controls.rs`](../contract/lower/src/controls.rs).
`buttonStyle` is a styleable host-policy prop whose complete vocabulary is the
schema's `buttonStyles` table. Unsaid, a button is the platform's default: plain
tinted text on iOS, a push button on macOS, bordered on the web and on the hosts
that paint their own (`contract vocab button`). This adds no tag or new
expression grammar. Platform looks and stand-ins are documented in
[LLP 1069.011](../llp/1069.011-native-buttons.rfc.md).

### Plain textarea decorations

`decorations` colours ranges of a plain `textarea`'s text that the app computes, as a
Bluesky composer colours @mentions, links and #tags: CSS's Custom Highlight API
([LLP 1114.007](../llp/1114.007-highlights-in-a-text-field.rfc.md)). Each range names a
`decoration` declaration, the app's `::highlight(name)` rule. The value stays a plain
string, and a decoration paints only: the field measures, wraps, edits and undoes exactly
as it does without one, so a declaration takes only what `::highlight()` admits, `color`,
`background-color`, `text-decoration` (`text-decoration-line`) and `text-shadow`; any other
property is a compile error. Declarations are scoped and imported like styles.

```text
decoration mention color="light-dark(#1083fe, #208bfe)"
component Composer
  state draft = "hello @alice"
  derive marks = TextDecorations(value=draft, spans=[TextDecorationSpan(name="mention", start=6, end=12)])
  action edit(value: string)
    draft = value
  view
    textarea id="composer" value=draft decorations=marks input=edit
```

`TextDecorations(value, spans)` names the text its spans were computed for, so an equal
span list for new text (`@alice` replaced by `@carol`) is still a new record. Spans are
UTF-16 offsets, clamped to `value`; where two overlap the later paints over the earlier.
Until the app's record for the field's current text arrives, as a person types, the host
paints the last record's spans moved across the edit as a DOM `Range` is (provisional).
Nothing is repainted while an input method composes. The web paints with
`createValueRange()` and `CSS.highlights` and leaves the `<textarea>` in place; a browser
without `createValueRange` paints nothing. macOS, iOS and tvOS paint TextKit 2 rendering
attributes; Linux and Windows paint colour runs with unchanged advances; Android and the
terminal paint nothing yet. Only the web paints `text-shadow`. `input` and
`textarea markup="markdown"` refuse `decorations`.

An action may call `rangeFrame(id, start, end)` for the `Geometry` of a textarea's UTF-16
range, plain or `markup="markdown"` (source offsets), as DOM's
`Range.getBoundingClientRect()` gives it: a collapsed range is the caret's rectangle where
the caret is painted (width 0, the height of the text there: its font's content area, as
Chrome measures it; an empty line's caret on that line, an empty field's at its start), a
wider one the union of its lines' text boxes (a line break's empty box counted only when
every box is empty), in viewport coordinates and not trimmed by scrolling or the field's
clip. Offsets are truncated toward zero; a non-finite or negative offset, a start past the
end, an end past the text, a missing or hidden field, composition, an `input`, and a host
with no reader (Android, whose field is the platform's own; the terminal; a browser without
`createValueRange` for a plain textarea) give `unavailable`. Linux and Windows read the
field as they last painted it: its text boxes (its glyphs across, their font's content area
down) through the transform it was painted with, bounded; a field placed by a perspective
is `unavailable` there (Windows by construction, the same presenter, not yet run on
Windows). The web measures an empty line's and an empty field's caret from a copy of the
field's text laid out in its box, as Chrome's `createValueRange` answers those at the text
before the line and at the viewport's origin. It reads the layout last drawn, so a `setRangeText` earlier in the same action is
not yet measured.

### Markdown: `markup`, `format`, `select`

`markup="markdown"` on a `text` or a `textarea` styles the node's own string as
Markdown ([LLP 1045](../llp/1045-markdown-editor.rfc.md)); `none` is the
default. It is not CSS: CSS has nothing for this. The value stays the Markdown
source, so storage, search and export are the string.

- `text note.body markup="markdown"` is the reader: headings, lists, quotes,
  code and links painted as one node, on every host.
- `textarea id="editor" value=draft input=edit markup="markdown"` is the editor:
  on the web, Markdown shown as it reads, with the syntax hidden away from the
  caret; on macOS and iOS, the platform's text view with the syntax dimmed and
  each link's `](destination)` hidden, as the web hides it. Linux
  edits it as a plain textarea. Without `markup` the same source is a plain
  textarea: the source mode.

`format(id, command)` and `format(id, command, argument)` edit the editor whose
`id` is given, at its selection, as one undo step reported through its `input`
as typing is. The commands are `bold`, `italic`, `code`, `strike`, `link` (the
argument is the URL), `heading` (`"1"` to `"6"`), `bullet`, `ordered`, `task`,
`toggleTask`, `quote`, `codeblock`, `footnote`, `figure` (the argument is the
media `src`), `indent`, `outdent` and `newline`; another name, or a missing
argument, changes nothing. A toolbar button takes `retainFocus=true`, so a press
leaves the editor focused and its selection where it was (on the Mac a click on any
button does, and on iOS a tap on one does, as UIKit's buttons do; the attribute is
what says it for the web).

The editor's `select` hands its action a `MarkdownSelection` when the selection
or the source changes: `formats`, space-separated (the inline command names,
`heading1` to `heading6`, `bullet`, `ordered`, `task`, `quote`, `codeblock`);
`mixed`, true when the selection spans different formats or links; `link`, the
common link target or `""`; `unavailable`, the commands that cannot apply
there, space-separated (most of them, inside a code block); and `atom`, the
selected atom's source when the selection is exactly one [atom](#inline-atoms-atom-atoms-atomfaces-atompress),
else `""`. It carries no offsets. A toolbar reads one format as `` ` ${s.formats} `.includes(" bold ") ``;
[Markdown Stress](../apps/markdown-stress/app.contract) is a whole toolbar and
link sheet.

The editor's `input`, `selectionchange` and `beforeinput` are a text field's
([form controls](#form-controls-radio-inputevent-setselectionrange)): their
`InputEvent` carries the source and its selection, in UTF-16 offsets into the
source, with the edit's `inputType`, `data` and `isComposing`. `beforeinput`
comes before the editor's own handling: a Return it cancels continues no list.
`setSelectionRange` and `setRangeText` take the same source offsets on the web,
macOS and iOS; each `setRangeText` is one undo step, as a `format` is, and
both are refused while an input method composes. On the web, where hidden
syntax is not drawn, a caret put inside it is drawn at its visible edge and
reads back there ([LLP 1045](../llp/1045-markdown-editor.rfc.md) D6).

`paragraphDirs` gives the editor's lines their base direction: one word per
source line, HTML's `dir` (`ltr`, `rtl` or `auto`), the first word the first
line's. A line feed ends a line, so a Markdown paragraph broken over three
source lines takes three words. A line past the last word takes the last, so
`dirs = "ltr rtl"` draws the first line left to right and every line after it
right to left. An editor where Arabic or Hebrew sits beside English binds the
words to what it derives from its value (one per `value.split("\n")`), by
whatever rule the app keeps, and so sends the next ones with each change. A
literal says one word, which every line takes: it never changes, so the
compiler refuses a list written there (`lower-paragraph-dirs-literal`). Until
the app's next words, each word stays with its line through edits, undo and
redo give back the words of the lines they restore, and a line an edit adds
takes the word above it. A line without a word takes the field's `direction`;
`auto` is the line's first strong letter outside its syntax and atoms. The
web, macOS and iOS draw it, a right-to-left line's list marker and quote on
its right (the web's quote bar; on Apple the quote's indent and its dimmed
`>`); Linux, Windows and Android, which show the source, ignore it
([LLP 1045](../llp/1045-markdown-editor.rfc.md) D11).

### Inline atoms: `atom`, `atoms`, `atomFaces`, `atompress`

An atom is a span of a Markdown source that the editor and the reader show as
one pill: a label that is not its source, an optional icon, a tooltip
([LLP 1045.000](../llp/1045.000-inline-atoms.rfc.md)). A caret cannot stand
inside it, a selection takes it whole, Backspace at its end (Delete at its start)
removes its whole source as one undo step, ⌥⌫ takes it with the spaces before
the caret, and a press selects it and then is the node's `atompress`. The value
stays the source; copy takes the source, ⌘⇧C the labels.

```text
atom citation link="cite:"
atom file link="relative"
atom mention prefix="@"
atom skill prefix="$" background-color="light-dark(#efe7ff, #2c2340)"

component Composer
  state draft = ""
  derive faces = [AtomFace(source="@src/main.rs", label="main.rs", icon="assets/file.svg", title="src/main.rs", color="", backgroundColor="light-dark(#e3f0ff, #1d3045)", direction="", ariaLabel="")]
  action pressed(e: AtomEvent)
    open = e.source
  view
    textarea value=draft input=edit markup="markdown" atoms="citation file mention skill" atomFaces=faces atompress=pressed
    text draft markup="markdown" atoms="citation" atompress=pressed
```

- `atom <name>` takes one form: `link="scheme:"`, any `[text](dest)` or
  `![text](dest)` whose destination has that scheme (compared without case);
  `link="relative"`, such a link whose destination has no scheme, between white
  space; or `prefix="@"`, one of the characters at the start of a word, then a
  bare run or a `"…"` string, followed by white space. A relative link and a
  prefix token are atoms only when a face names their source; a scheme link
  always is. It may take `role="link"` (a button by default) and `color` and
  `background-color`; the pill's other metrics are the platform's
  (`lower-atom-style`). It is scoped and imported as `style` is.
- `atoms="a b"` on a `text` or `textarea` with `markup="markdown"` lists the
  kinds it recognizes (`lower-atom-field`, `lower-atom-unknown`); two kinds with
  one scheme, a shared prefix character, or both relative are refused
  (`lower-atom-duplicate`).
- `atomFaces` takes `AtomFace(source, label, icon, title, color,
  backgroundColor, direction, ariaLabel)` records, keyed by an atom's whole source: equal
  source is one atom, and new text is a new key. An empty label is a link's text
  or a token without its prefix and quotes; the icon is an image source (the web
  draws a URL or asset, not yet a `symbol:`). `color` and `backgroundColor`
  paint that atom over its kind's `color` and `background-color`, or are empty
  for the kind's. Each is a colour as a bound `color` takes one: `light-dark()`,
  a theme entry's name, a role, a `-exact-platform-color()` only as one of the
  app's literals, or a colour in a wide space (`color(display-p3 …)`), which
  every host with pills draws in that space. A colour in a standard space the
  platforms ship (`color(--dci-p3 …)`) is drawn by Apple and refused on the web,
  and one in a profile the app declares with `color-profile` is refused on every
  host. One that names a token follows `setMode`, a pin and a token's new value;
  when that value is no colour, faces the token painted stay as they were, where
  a themed row would be unset, and the journal says so. A role's web form
  (`var(--exact-separator, …)`) is that role, whatever its fallback holds but an
  unadmitted `-exact-platform-color()`. Any other value that holds
  `currentcolor`, `inherit` or another cascade keyword, or a substitution
  function (`var()`, `env()`, `attr()`, `if()`, `inherit()`), is no colour on
  every host. So is a colour the kernel does not read, on the native hosts and
  the web's wasm target, and one the build's colour map or the browser refuses,
  on the JS target. A colour that is none leaves the kind's paint, and the
  journal says so. A pill lays out its icon and then its label in its line's
  direction, so in a right-to-left line its icon is on the right; `direction`
  (`ltr`, `rtl`) pins its own, and empty follows the line. Assistive
  technology reads the label as the pill's name and `title` as its description;
  a nonempty `ariaLabel` is the name instead, as `aria-label` names a node, so
  a pill can say more than it draws (`ariaLabel="Image attachment, a.png, 2 MB"`
  over a label of `"a.png 40%"`).
- `atompress` hands its action an optional `AtomEvent`: `kind`, `source`,
  `href` (a link's destination as written), `start`, `end` (UTF-16) and the
  modifiers. It never navigates. In the reader an atom is a tab stop that
  Return or Space presses (a link atom, Return only); in the editor it is not.
  `rangeFrame` over its range is its pill, in the editor and the reader alike,
  where a popover anchors; elsewhere in a reader's text it is unavailable, as
  it is in a macOS paragraph that lays out off the main thread (LLP 1072): 64
  KB of text, or 16 KB in 256 runs or more, a Markdown text being one run per
  piece.

The web, macOS and iOS draw atoms in the editor and the reader (on Apple an
icon draws only for `symbol:sf/<name>`, and UIKit's smart delete takes one of
the two spaces an atom's deletion leaves). VoiceOver on macOS and iOS reads
an atom's label in place and its role only from its own element, which the
next-item command passes over on macOS and reaches after the field on iOS
(LLP 1045.000 §10 Q10). Linux, Windows and Android show the source in the
editor and the link text in the reader, with no `atompress`. The driver's
`state atoms` lists each one: its node, kind, source, range, label and frame from the node's top
left, where `tap <node> at <x> <y>` presses it.

### Keep-awake: `wakelock`

`wakelock` on any element takes `"screen"` or `"none"` (the default), literal
or bound ([LLP 1125](../llp/1125-a-transport-midi-and-the-small-pieces.rfc.md)
D11). While an element that is mounted and displayed (no ancestor
`display: none`, under no unselected tab and no screen a stack keeps under
its top; the one a sheet covers is shown) says `"screen"`, the display does
not dim or lock for idleness. Any other literal word is refused (`lower-attr-value`, naming the
two); a computed one is journaled and holds nothing. `keepScreenOn`,
`idleTimerDisabled` and `keepAwake` are refused with this name. The agent
reads `state.wakelock` (`{held, by}`); [the reference](reference.md#keep-the-screen-awake)
has each host's lock.

### Orientation lock: `orientationlock`

`orientationlock` on any element takes a Screen Orientation API lock type
(`"portrait"`, `"landscape"`, `"portrait-primary"`, `"portrait-secondary"`,
`"landscape-primary"`, `"landscape-secondary"`, `"natural"`, `"any"`),
`"current"` or `"none"` (the default), literal or bound
([LLP 1133.029](../llp/1133.029-screen.rfc.md)). It holds while the element
is displayed, by `wakelock`'s rule, and outside every `dialog` and popover.
Any other literal is refused (`lower-attr-value`, naming the words); a
computed one is journaled and holds nothing. The bake refuses a lock inside
a `dialog` or popover (`orientationlock-in-top-layer`) and a literal type
that shares no orientation with the manifest's `orientation`
(`orientationlock-outside-manifest`). `screenOrientation`, `lockOrientation`,
`orientation`, `requestedOrientation` and `supportedInterfaceOrientations` are
refused with this name. The agent reads `state.orientationlock`;
[the reference](reference.md#lock-the-screen-orientation) has the rules
and each host's lock.

## Events

An event binding is an action reference or partially applied action. Captured
arguments precede the event payload. The table contains all 61 handler names.
Numeric multi-argument payload ordering should be copied from the feature's
working fixture, not inferred from JavaScript's Event interface.

Six events bubble, as in the DOM: `key` and `keyup` from the focus,
`focusin` and `focusout` from the element gaining or losing focus,
`pointerdown` and `pointerup` from the node pressed, and `wheel` from the node
under the pointer, each to every ancestor that declares it, innermost first
(`keycapture` goes outermost first). An action that calls `stopPropagation()`
stops that event at its handler, as DOM's
[`Event.stopPropagation()`](https://dom.spec.whatwg.org/#dom-event-stoppropagation)
does: no handler after it hears that event. The stop is that event's alone. A
stopped `pointerdown` still sends its `pointerup` to every node it passed, and
the pointer's moves stay with the node that holds them. The event's default
action still happens: only `preventDefault()` stops a key's typing, a
wheel's scroll or a press's focus move, and a stopped `pointerdown` or
`pointerup` leaves the click's `press` alone. A `pointerdown` action that
calls `preventDefault()` keeps the focus where it is, places no caret and
starts no selection, as Chrome's does, by mouse and by touch; the `press`
still fires, and an action's own `focus()` still moves the focus. (On iOS a
field that already has the focus still places its caret where the touch
lifts; LLP 1005 §3.) A formatting toolbar keeps its editor focused so:

```text
action keep(e: PointerEvent)
  preventDefault()
button pointerdown=keep press=bold …
```

`press` is DOM's `click` stopped at its handler: a press goes to the nearest
node from the one pressed up that takes a press (a `press` handler or a
link), and to no node above it. Content in an open popover or a modal
`dialog` presses the popover's or dialog's authored ancestors, as a click
bubbles out of the top layer, never a node only under it: a modal dialog's
ancestors take that press though the dialog makes them inert, while an
authored `inert` still silences them. That ancestor's own box is seldom
under the pointer, so the press lands as the content it went down on judges
its own: a pointer that leaves that content before it lifts presses nothing.
A press that reaches an invoker (`commandfor`, `popovertarget`) behind a
modal dialog runs its `press` handler, and opens or closes its target only
where the target is inside the modal's subtree; a target the modal makes
inert stays as it is, as a `contextPopover` on an ancestor behind the modal
opens no menu.

A text field, a checkbox or another form control takes a tap or a click
itself, so an ancestor's `press` does not fire, in the page or in a dialog.
That is the platform's rule on macOS and iOS (a hand-built screen's control
consumes its touch, [LLP 1115](../llp/1115-write-the-web-ship-the-platform.principle.md));
on the web the click bubbles from the control to the ancestor. A `press`
written on the control itself gives the same result on every host. A
`button` with no `press` of its own presses the nearest ancestor's on every
host.

| Payload appended to captured arguments | Handler names |
| --- | --- |
| A string, then optionally an `InputEvent` | `change`, `input` on a text field, textarea, `select`, date or time input, and `type="radio"` (the radio's `value`); an action taking one more parameter also hears the [target](#form-controls-radio-inputevent-setselectionrange) |
| One string | `message`, `error` |
| A string, then optionally a `KeyboardEvent` | `keycapture` (capture-phase `keydown`), `key` (bubble-phase `keydown`) and `keyup`: the key's name; an action taking one more parameter also hears the [modifiers, the physical key and whether it repeats](#keys) |
| Two numbers, then optionally a `ScrollEvent` | `scroll`: left and top; an action taking one more parameter also hears the scroller's extents (below) |
| Zero or one `IntersectionObserverEntry` | `intersect`: `isIntersecting` (bool) and `intersectionRatio` (number), mount then enter/leave at threshold 0; currently web only |
| Two numbers, then optionally a `DOMRectReadOnly` | `resize` given an action: the content box's width and height; an action taking one more parameter also hears its `contentRect` (below). A string `resize` is CSS's property |
| One boolean | `hover`; `fullscreenchange` (whether the video is now full screen) |
| One boolean, then optionally an `InputEvent` | `change`, `input` on a checkbox or `switch` |
| One number | `timeupdate`, `durationchange` |
| One number, then optionally an `InputEvent` | `change`, `input` on `type="range"` |
| One `Picked[]`, then optionally an `InputEvent` | `change`, `input` on `type="file"` |
| One `MarkdownSelection` | `select` on the Markdown editor (`textarea markup="markdown"`) |
| One `InputEvent` | `select` on a text field or any other `textarea` ([form controls](#form-controls-radio-inputevent-setselectionrange)) |
| Two numbers | `pan`, `panrelease`, `heightrelease` |
| A string, an `option<string>`, then optionally a `ReorderEvent` | `reorderdrop`, on a vertical `list virtualized=true` only: the dragged row's key, then the key it lands before (`none` at the end); an action taking one more parameter also hears `ReorderEvent { from, to }`, the two lists' `id`s (equal within one list; [LLP 1094](../llp/1094-dropping-across-lists.rfc.md) D2) |
| Four numbers | `transformgeometry` |
| Six numbers | `transformrelease` |
| Special: zero or one location string, no captured args | `navigate` |
| Zero or one `PointerEvent` (the action takes it or leaves it) | `pointerdown`, `pointerup`, `pointermove`, `contextmenu` (UI Events makes it one: where the secondary click or long press was; a keyboard's menu key targets the focus) |
| Zero or one `WheelEvent` (the action takes it or leaves it) | `wheel` |
| Zero or one `DragEvent` (the action takes it or leaves it) | `drop`, `dragenter`, `dragleave` |
| Zero or one `ClipboardEvent` (the action takes it or leaves it) | `copy`, `cut`, `paste` ([clipboard](#clipboard)) |
| Zero or one `FocusEvent` (the action takes it or leaves it) | `focusin`, `focusout`: `target` is the focused element's `id` (or `""`); `relatedTarget` is `some(id)` for the other element or `none` |
| Zero or one `Selection` (the action takes it or leaves it) | `selectionchange`, on a `text` ([text selection](#text-selection)) |
| Zero or one `InputEvent` | `selectionchange` and `beforeinput`, on a plain text field |
| Zero or one `MediaSessionActionDetails` (the action takes it or leaves it) | `seekbackward`, `seekforward`, `seekto`, `previoustrack`, `nexttrack`, `stop`, on an `audio` or `video` with `metadata=` ([media session](#media-session)) |
| Zero or one `MIDIMessageEvent` (the action takes it or leaves it) | `midimessage`, on an element with `midiinputs=` ([MIDI](#midi-exactmidi-sendmidi-midimessage)) |
| Zero or one `MouseEvent` (the action takes it or leaves it) | `press`: the modifier keys held, `shiftKey`, `ctrlKey`, `altKey`, `metaKey` (a shift-click, a ⌘-click; all false from a keyboard or assistive activation), and where the pointer was, `clientX`, `clientY`, from the viewport (0, 0 from a keyboard or assistive activation, as Chrome's; a popover's `left`/`top` can open it there, LLP 1021 D2) |
| None | `beforeunload`, `cancel`, `close`, `scrollend`, `focus`, `blur`, `submit`, `load`, `dblclick`, `swiperight`, `refresh`, `loadedmetadata`, `play`, `playing`, `pause`, `ended`, `waiting`, `seeking`, `seeked`, `ratechange`, `volumechange`, `canplay`, `reachstart`, `reachend` |

On text fields and textareas, `input` reports every edit. An `input` handler
should update raw draft state so live search, validation and button enablement
follow typing. `change` reports a committed value: Enter or blur for a text
field, the end of editing for a textarea. Its handler should normalize the
draft. Use `submit` for explicit submission. Checkboxes, switches and selects
use `change` when a value is chosen.

A dialog's `close=action` runs after dismissal and focus restoration on the web,
macOS and iOS, whether an action, an invoker button or Escape closed it. It takes
no payload. Closing an already-closed dialog fires nothing; removing a dialog or
resetting the session does not fire `close`.

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

### Media session

`metadata=MediaMetadata(title=…, artist=…, album=…, artwork=…)` on an `audio`
or `video` claims the platform's media session (Now Playing, the media keys, the
lock screen; [LLP 1098](../llp/1098-the-media-session.rfc.md)). `MediaMetadata`
is a compiler shape an app builds as it builds its own records (every field a
string, each named once; `artwork` is one image's source), so a `fn` may return
one; an app's own `shape MediaMetadata` or `fn MediaMetadata` is refused. The
element's six actions append a `MediaSessionActionDetails { action: string,
seekOffset: number, seekTime: number, fastSeek: bool }` to an action that takes
one: `seekOffset` is the platform's, else the element's `seekbackwardOffset` or
`seekforwardOffset` (seconds, greater than 0, default 10), 0 for the other four;
`seekTime` is `seekto`'s. An action or an offset on an element without
`metadata=` is `lower-media-session`. The platform's play and pause act on the
element, whose own `play` and `pause` events report them; the session's position
and playback state are the player's. Of several claimants, the one that most
recently started playing owns the session (else the latest mounted).
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
list appearance="none" virtualized=true scroll-start="end" scrollFollowEnd=true scroll=moved …
```

On a virtualized list, the record also carries `firstVisibleKey: option<string>`
and `firstVisibleOffset: number`: the first row intersecting the port, and the
signed distance in CSS pixels from its wrapper's start to the port's start,
on the list's axis. Keys are text (numeric and boolean keys use their text
representation). The snapshot uses the event's offset and the list's latest
measured/estimated sizes; zero-size rows are skipped. An ordinary scroller,
an empty list, or a port showing only padding reports `none` and `0`.

The app can save these two fields by document or conversation and restore
after mounting the list with `scrollIntoView("feed", key, offset=within)`.
`offset=` is a finite number and belongs only to the list form. It places the
port that many pixels into the keyed row wrapper, overriding that list's
`block`/`inline` alignment and scroll-padding, clamped to the scroll range.

A virtualized list mounts the rows near its port and unmounts the rest.
`keep-mounted=` names rows that stay mounted wherever the window is: a list of
up to 64 keys as the `for`'s `key=` gives them, each possibly an `option`
(`none` names no row), computed or literal. A screen reader moves only through
mounted rows, so a transcript keeps its first and last message,
`keep-mounted=[first(ids), ids.at(-1)]`; a row playing audio keeps its player.
A key the list does not hold is passed over; another value keeps none and
says so in the journal (LLP 1010 §6.11).

`scroll-padding` (or its four physical sides) insets any scroll container’s
alignment port. `scroll-margin` expands the target element’s border box for
`scrollIntoView`; its signed lengths change neither layout nor scroll range.
Padding percentages use the corresponding scrollport dimension on ordinary
scrollers; a virtual list still requires resolvable main-axis lengths.
Negative offsets preserve padding showing before the row. With `row=`, only
the inner list takes the offset; the outer row aligns normally. Unmounting a
top-level list saves nothing automatically, with either `scroll-restoration`
value.

As on the web, the event comes when the offset changes (a follow of the end
moves it); content that grows below a reader who is not following it changes
no offset and sends none.

`scrollend`, on a `scroll` or a `list`, comes once a scroll that moved the
offset is over, after its last `scroll`: the finger lifted with nothing thrown,
the momentum or a smooth scroll stopped, or (a jump, a mouse wheel's notches)
the offset still for 100 ms on Apple's hosts. It takes no payload. An app that
needs to know a fling has stopped hears it, instead of restarting a timer on
every `scroll`. Linux and the shared Android Presenter complete notches and
instant jumps at the next delivery point; reader samples use 100 ms quiet.
Delivery uses the existing host clock after its runner work, so a late wake may
run a later timer first. Android device qualification remains open (LLP 1005,
“Linux, first step”).

`intersect=action` offers an optional final `IntersectionObserverEntry` after
any authored arguments. The record contains `isIntersecting: bool` and
`intersectionRatio: number`. The nearest ancestor CSS scroll container is the
root, otherwise the viewport; threshold is fixed at 0. `intersect-margin` is a
literal rootMargin string (one to four px/% lengths, default `"0px"`). It fires
on mount and enter/leave, not every ratio change; removal stops observation
without a leave event. Both web targets implement it; native delivery remains
owed (LLP 1005, "Intersection visibility").

`resize` is two things, told apart by its value, as nowhere else: a string
(`resize="none"`) is CSS's `resize` property, and an action is the element
resize event, `ResizeObserver`'s. Its action hears the content box's width
and height after layout: once the element is first laid out, then whenever
its content box changes size (a commit that leaves it alone says nothing; one
under `display: none` reads 0 by 0). A box that straddles the columns of a
multi-column flow is one column wide and as tall as its pieces end to end, as
Chrome reports it, while `frame()` answers its union (LLP 1093 D11). One more
parameter hears the entry's `contentRect`, a `DOMRectReadOnly`: `x` and `y` the padding's left and top,
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
also carry `navigationKey` (the navigation root; its `navigationBack`, naming the
Back control, is optional). Transform geometry/release
bindings are required as a pair. See
[`handler_arity`](../contract/analyze/src/lib.rs) and the corresponding corpus/tests.

### Pointer

`pointerdown`, `pointerup` and `pointermove` are DOM's, on every host. A
touch or a button going down (any: `buttons` says which, 2 for a right-click)
is the `pointerdown` of every enabled node from the one under it up that
declares it, innermost first, as `key` and `wheel` bubble, before any gesture
decides; a press inside a popover or a modal dialog reaches its authored
ancestors, never a node only under it. When the pointer lifts or is cancelled (a cancel is an
up), each node the down passed that declares `pointerup` hears it, innermost
first, wherever the pointer is, before the click's `press`. The innermost
enabled node that hears any of the three holds the pointer: its
`pointermove`s follow the pointer wherever it goes until it lifts, and a
`pointermove`, or a `pan` that takes the contact, takes no ancestor's down
or up. A node disabled when the pointer goes down hears none of them, and
its ancestors still do. A handler's `stopPropagation()` stops the down or up
it hears there ([Events](#events)). A free pointer (a mouse or a pen
hovering, no button down) moving over a node is that node's `pointermove`
too, the innermost hearing it; a touch has none. Moves go out at most once a frame,
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
A Control-click there is a secondary click too: where a `contextmenu` or
`contextPopover` answers it, it holds the primary button and Control, and
nothing presses. A secondary click in a modal dialog reaches an ancestor's
`contextmenu` behind it, but that ancestor's `contextPopover` stays shut,
since the modal makes it inert; a press bubbling to an invoker behind the
modal follows the same rule ([Events](#events)).
On Apple hosts, the unmodified `ContextMenu` key and Shift+F10 open it at the
focused node's centre, after `key` handlers run, with `buttons=0` and
`pressure=0`. A `key` handler's `preventDefault()` cancels it. The nearest
`contextmenu` handler runs before its `contextPopover` opens; either declaration
is sufficient. A text field keeps its editing menu. Drive it with
`type "row" key "ContextMenu"` or `type "row" key "Shift+F10"`. UIKit presents
keyboard-invoked actions through its native menu without the long-press preview.
Disabled controls and inert nodes do not open menus. On macOS a request that
no node answers, from the key or from VoiceOver's show menu, gets the menu a
right-click at that point would show, as Chrome shows its own menu then: the
menu of the view drawn there or above it (selected text's), else whatever
answers past the session's view (an embedder's or an app's responder). No
`pointerdown` or `contextmenu` fires, and selected text keeps its selection.
With nothing focused and text selected, the key opens at the selection's first
line, as Chrome's does. The web shows the browser's own menu; the other hosts
have no page menu to show.

`wheel` is DOM's: a wheel's turn or a trackpad's scroll over the node, heard by
every node from it up that declares it, innermost first. Its `WheelEvent` is
`offsetX`, `offsetY`, `deltaX`, `deltaY` (CSS px; positive scrolls down and
right), `deltaMode` (0 pixels) and the four modifiers. A trackpad's pinch is a
wheel with `ctrlKey` and `deltaY` of -100 × its magnification, as browsers
deliver one. An action that calls `stopPropagation()` stops the wheel at its
handler and leaves the scroll alone; one that calls `preventDefault()` keeps
the scroll from happening (web, macOS, Linux; iOS has none):

```text
action wheeled(e: WheelEvent)
  if e.ctrlKey or e.metaKey
    zoom = zoom * (1 - e.deltaY / 100)
    preventDefault()
scroll wheel=wheeled …
```

`drop` is DOM's `drop` of files dragged in from outside the app (Finder, the
desktop) onto the innermost node that declares it: its `DragEvent` is
`offsetX`, `offsetY`, `files` (a `string[]` of `doc:` handles minted as a
picker's are, [LLP 1069.010](../llp/1069.010-the-mac-as-a-document-platform.rfc.md)
D1, readable under `fs.read doc:/`) and the modifiers. Files of any type are
taken, and a dropped folder is a folder's handle (`fs.stat` says
`isDirectory`, `fs.readdir` lists it); the app never learns an absolute
path. A file drag over an `input` or a `textarea` inside such a node (or
declaring `drop` itself) is the node's, as on the web, where its listener
prevents the field's own drop: the field inserts no path text. A text drag,
and a file drag over a field with no `drop` above it, stay the field's. Web
(a browser without `getAsFileSystemHandle` hands read-only copies) and macOS;
iOS, Linux, Windows, Android and the terminal have none. The driver's `tap
<target> drop <path…>` drags files and folders in.

`dragenter` and `dragleave` are DOM's, as a node hears them when it ignores
moves between its own descendants (the `currentTarget.contains(relatedTarget)`
test a drop overlay makes): a node that declares one hears `dragenter` when
a drag of files from outside the app comes over its box, and `dragleave` when
it leaves the box or is cancelled. A drop sends `drop` and no `dragleave`, as
the DOM's does: the drop's action clears what the enter drew. Their
`DragEvent` has the point and the modifiers and no `files`, which the DOM
keeps from a page until the drop. They are heard whether or not a `drop`
will take the files, so an overlay can say where they would land:

```text
state over = false
state paths = []
action entered
  over = true
action left
  over = false
action took(e: DragEvent)
  over = false
  paths = e.files
column dragenter=entered dragleave=left drop=took position="relative"
  textarea placeholder="Message"
  if over
    column position="absolute" inset=8 pointer-events="none" border-width=2 border-style="dashed" align-items="center" justify-content="center"
      text "Drop files to attach" role="status"
```

Web and macOS, as `drop`. The driver's `tap <target> dragenter <path…>`
brings a drag of those files over the target and leaves it there (for a
screenshot); `tap <target> drop <path…>` drops it, and `tap <target>
dragleave` takes it away undropped.

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
`key` node does. With nothing focused they go to the first root, as the
DOM's go to `<body>` (web, macOS and iOS; the Edit menu's item is enabled
while the root has the handler; a Mac sheet's own window holds none). On the web and macOS Paste and Match Style
(⌥⇧⌘V on the Mac) is a `paste` too, as Chrome's is: an app that treats it
apart tells by the key before it, as a page does. In a field whose node or
an ancestor hears `paste`, Paste is enabled on the Mac while the clipboard
holds only files or image data, which the field cannot take itself; the
paste carries them as `files` and inserts nothing. An action that takes one more parameter gets a
`ClipboardEvent` whose `text` is the clipboard's plain text: what is pasted,
and empty on `copy` and `cut`, as the DOM's is until a listener sets it — the
action writes the clipboard with `copyText`. Its `files` is the paste's
`clipboardData.files`, each a `doc:` handle as a dropped file's (a copied file
or folder, any type); image data with no file behind it (a screenshot, an
image copied from a page) arrives as one `image.png`. A paste of text alone
has none, and `copy` and `cut` none. Web, macOS and iOS; Linux, Windows,
Android and the terminal paste text only, and tvOS has no pasteboard
(`files` is empty). In an `input` or `textarea`
the event comes first and the field's own cut, copy or paste follows,
unless the action calls `preventDefault()`, which cancels it as the DOM's
does (a paste then inserts nothing): on the web and on macOS and iOS alike,
where the field's editor fires the three (a password field's on macOS
fires none). The driver's `type <id> paste <text>` delivers a paste
carrying that text, `type <id> paste --file <path…>` one carrying those files
(and folders), `type <id> paste --image <png>` one carrying that image's data
with no file, and `type <id> copy` and `type <id> cut` the others,
without touching the system clipboard; at a field, an unprevented paste
inserts the text.

```text
action pasteAt(cell: string, e: ClipboardEvent)
  send pasted = pasteCells(cell, e.text)
column key=move paste=pasteAt(selected) copy=copyCells cut=cutCells
```

To write an image, data code imports `@exact/clipboard` (LLP 1133.006): the
Clipboard API's `write([new ClipboardItem({"image/png": path, "text/plain":
text})])`, with an `app:/` path to a PNG (or at most 2 MiB of PNG bytes)
and `writeText(text)`. The host writes a new PNG of the image's pixels, frame
0 of an animation; convert another format first with
`storage.fs.compressImage(…, {type: "image/png"})`. Web, macOS and iOS. The
app's Contract names the package with a `use` (`use ClipboardItem from
"@exact/clipboard"`), which links it. Under the agent neither this nor
`copyText` touches the person's clipboard: the write lands on the session's
own pasteboard (in memory on the web), and `state.extensions.clipboard.implementation`
shows the last one (`EXACT_DEVICE=real` writes the real one on Apple).

### Text selection

`selectionchange` on a `text` is the web's `selectionchange`, per element:
when the part of the reader's text selection inside that paragraph changes —
a drag, a double or triple click, select-all, a click that clears it — an
action that takes one more parameter gets a `Selection`: its `text` (the
selected source text), its `rects: Geometry[]`
(the selected fragments in viewport coordinates), and its `start` and `end`,
UTF-16 offsets into the node's own text as written (white space before CSS
collapses it, a `text`'s inline children joined in order, and Markdown line
breaks counted as newlines). Inline IDs use offsets within that inline text.
Nothing selected
there is `text: ""` with `start == end == 0` and `rects: []`; it fires while a drag moves,
once per change. The web and macOS select text; iOS selects text its author
made selectable (`user-select="text"`), by a long press, a double tap and the
selection's handles; Linux has no text selection on a `text`, so it never
fires there. In a `user-select="all"` element one click (on iOS a long press
or a double tap) selects the whole element, and a selection that reaches
into it takes all of it.

On web and macOS, `removeAllRanges()` clears the reader's selection. Each
paragraph that held a part reports its empty selection. The documented
extension `setSelectionRange(id, start, end[, direction])` also selects a
`text`: offsets are UTF-16 in its source text, clamped as for a field; an end
before the start collapses at the end. Requested source boundaries survive
CSS whitespace collapsing. `backward` puts the anchor at the end,
as `Selection.setBaseAndExtent` does. The command replaces the page's previous
selection and fires `selectionchange` for changed parts without focusing a
field. An unchanged part does not fire again. These two commands on reader
text remain unsupported on iOS and Linux; iOS selection events carry an empty
rectangle list.

Rectangles are snapshots at that event, not live layout facts. On the web they
are the paragraph's selected `Range.getClientRects()`; macOS uses the same
CoreText fragments as its selection highlight and inline-run geometry. Their
`provisional` and `unavailable` flags are false; an empty selection has no
rectangles. Native paragraphs rendered by the reader worker currently return
an empty rectangle list: their caret geometry is not available synchronously.
Scrolling alone does not emit another selection event.

```text
action mark(para: string, s: Selection)
  selection = Excerpt(para=para, from=s.start, to=s.end, text=s.text)
text para.body selectionchange=mark(para.id)
```

### Progress: `progress`

`progress value=… max=…` is HTML's determinate progress bar, each platform's
own: `UIProgressView` (`.default`) on iOS, tinted by `accent-color` (UIKit's
tint where none is said); a determinate `NSProgressIndicator` bar on macOS
(small, regular where the box holds its 20 points; AppKit gives it no tint);
the browser's `<progress>` on the web; and on Linux a painted bar, a 4-pixel
track filled with the accent. `max` is 1 unless said (a number above 0), and
`value` is held to 0 and `max`, as HTML holds it; both take numbers, bound or
literal. It is as wide as what holds it and `1em` tall: the compiler gives it
`width: 100%`, as it gives `input type="range"`, since neither platform control
has a width of its own (LLP 1115.000 D10.2). That is an ordinary CSS row, so a
`width` you write replaces it, and in a `row` the bar or slider takes what the
row's other children leave and shrinks below its own width to do so, while a
label of several words beside it shrinks too and wraps: give the control
`flex=1 min-width=0` and the label keeps its line. Where the percentage has
nothing to resolve against (a box sized by its content), or under
`width="auto"`, each is its intrinsic size: a progress `10em` by `1em` (160 × 16
at 16 px), a range 129 wide. The bar is centred in the box at its own thickness. Its role is
`progressbar`, its value the platform's (UIKit's "25%", AppKit's value of its
maximum, the DOM's attributes); name it with `aria-label`, and `aria-valuetext`
gives the words a reader speaks instead. A change of value eases on iOS, except
under the agent's clock.

`progress` with no `value` is HTML's indeterminate progress, shown as the
platform's activity indicator: `UIActivityIndicatorView` on iOS (`.medium`,
`.large` where the box's shorter side is 37 points or more), a spinning
`NSProgressIndicator` on macOS (small, regular from 32 points), and on the web
and Linux a spinner the host draws, since HTML's own indeterminate progress is
a bar (the one deliberate divergence, [LLP 1069.001](../llp/1069.001-form-controls.rfc.md)).
It is a 20 × 20 box until `width` or `height` sizes it, and the indicator is
centred in it. Its own `color` colours it on iOS, the web, Linux and Android's
reader, else an authored `accent-color` (on it or around it, or the design's
`AccentColor`), else the platform's ink (an inherited colour never does: LLP
1115 D4, LLP 1121 D9); macOS draws the system's colour, as AppKit gives a
spinner no tint. Its role is `progressbar`,
busy (`aria-busy`); name it with `aria-label`. It turns while it shows, and
stops where it is hidden or gone; under the agent's held clock it shows one
still frame (on the web, the frame at the agent's time, as every CSS animation;
Linux paints one still frame always). A `max` with no `value` is refused (it is
the bar's), and so are children, `type`, and `aria-valuenow`, `aria-valuemin`
and `aria-valuemax` (`value` and `max` are its range).

```text
progress value=glasses max=8 aria-label="Water today"
progress aria-label="Loading"
progress width=37 height=37 color="#1083fe" aria-label="Loading posts"
```

### Form controls: radio, `InputEvent`, `setSelectionRange`

`input type="radio"` is HTML's (x2apps survey #2). Its `name` is its group:
the radios of one non-empty `name` in the window are exclusive, and a radio
with no `name` is a group of its own. Choosing an unchecked radio checks it and
unchecks the rest at once, then fires `input` and `change` on it, carrying its
`value` (`on` when it has none); choosing the checked one fires nothing. With
a radio focused, ArrowDown and ArrowRight move the focus and the check to the
next enabled radio of its group, ArrowUp and ArrowLeft to the previous,
wrapping, each move an `input` and a `change`; Space checks the focused one.
`checked` is controlled as a checkbox's: after the action, every radio shows
its bound `checked` again, so an action that writes nothing snaps the group
back. The driver's `type <radio> true` checks it as a click does; `false` is
refused, a radio being unchecked only by checking another.

```text
action pick(value: string)
  color = value
for c in colors key=c
  input type="radio" name="color" value=c checked=color == c change=pick aria-label=c
```

`input` and `change` hand an action that takes one more parameter an
`InputEvent`: the target's own fields as the event leaves it, by the DOM's
names — `value` (a checkbox's or a radio's own `value`, `on` when it has
none; a range's number as text), `checked`, and a text field's
`selectionStart`, `selectionEnd` (UTF-16 offsets into its value) and
`selectionDirection` (`forward`, `backward` or `none`); a control without a
text selection reports 0, 0 and `none`. `inputType` names the edit (for example
`insertText`, `insertFromPaste`, `deleteContentBackward` or `historyUndo`).
`data` is `option<string>`: inserted text, or `none` for deletion and history.
`cancelable` and `isComposing` report whether the event can be canceled and
whether it belongs to an IME composition. Events without edit metadata
(`change`, `select`, `selectionchange`) report `""`, `none`, `false`, `false`.
After typing, the caret sits after
the typed text; the direction then is the platform's (Chrome's varies with
the input), so test the offsets. A text field's `select` (an `input` that is
no control, a `textarea` that is not the Markdown editor) is HTML's: it fires
when the person selects text (a non-empty selection, each time it is
extended), and when `setSelectionRange` changes the selection, collapsed
included; typing and a plain caret move do not fire it. Its payload is the
`InputEvent`.

While an input method composes in a text field or a textarea (Japanese kana
before its kanji, pinyin, a Korean syllable), `input` fires at every change
with the composing text in the value, as the DOM's does, on every host that
composes (the web, macOS, iOS, Android): "に", "にほ", "にほん", then "日本".
Contract has no `compositionstart` or `compositionend` handlers. An input
action can inspect `InputEvent.isComposing` to distinguish composition edits
from other edits; the value includes the current composing text. On macOS and
iOS the commit of what is composed as it stands changes no text and is not
another `input`;
`change` is still the end of the editing. An action that writes the value it
was handed back (a controlled input) changes nothing in the field, and the
composition goes on. One that writes a different value (trimmed,
upper-cased, cut to a length) does not interrupt the person either: on every
host that composes, the value is held while the text composes and lands when
the composition ends, committed or cancelled, so the rewrite shows a moment
later (the last value written, where there were several).

`setSelectionRange(id, start, end)` and `setSelectionRange(id, start, end,
direction)` are the field's own method, by its `id`, after the commit's tree
is in place (so a `value` written beside it is there first). It does not
focus the field; the field keeps the selection, and the next key typed there
replaces it. The offsets are clamped as the DOM's are (`-1` is past the end).

On **web (JS and wasm), macOS, iOS and Linux**, `setRangeText(id, text)` replaces the
current selection; `setRangeText(id, text, start, end[, selectionMode])`
replaces the given UTF-16 range. `selectionMode` is `preserve` by default,
or `select`, `start`, `end`, following HTML. It does not focus the field,
apply `maxlength`, or create an undo step. Apple clears the edited field's
existing undo history when its value changes. Invalid ranges, modes, unsupported
input types and missing ids log a refusal without editing. On the Markdown
editor (web, macOS and iOS) the offsets are the source's and each call is one
undo step ([Markdown](#markdown-markup-format-select)); Linux refuses it there.
Windows, Android and terminal do not support this command yet.

**Deliberate deviation (Charlie, 2026-10-12):** unlike the DOM method,
Exact fires one noncancelable `input` after a successful replacement, even
when the value is unchanged. Wire `input` to update your bound value. Its
`InputEvent.inputType` is `insertReplacementText`. This type also describes
hatch replacements and browser spelling corrections; it does not identify
a `setRangeText` call uniquely. The event's `data` carries the replacement
text. The browser still delivers its own `select`/`selectionchange` events. Chrome delivers
`select` after the added input, only when the selection changes; native
hosts queue replacement selects after the action's commands and report the
field's current value and selection at delivery. macOS Chrome resets direction
to `none` on replacement; Apple follows it. URL inputs strip leading and trailing
ASCII whitespace after removing CR/LF. Contract's
field `selectionchange` also reports collapsed caret moves through an optional
`InputEvent`, independently of a reader's `Selection`. It is queued and
coalesced per field, and reads the current value and selection at delivery.

`beforeinput=action` takes an optional `InputEvent` describing the old value
and selection and the proposed `inputType` and `data`. `preventDefault()`
keeps a cancelable edit from changing the field. A subsequent `input`
describes the applied edit and is noncancelable. IME composition edits are
noncancelable; `preventDefault()` cannot interrupt them. `setRangeText`
continues to emit only its recorded extra input, without beforeinput.
The web forwards the browser's metadata. AppKit and Linux intercept ordinary
field edits and paste before applying them; AppKit's undo/redo responder
commands are cancelable too. UIKit's delegate observes ordinary edits,
but undo operations that bypass its delegate remain a host limit. Linux
has no native field undo history. The Markdown editor's events are these,
over its source ([Markdown](#markdown-markup-format-select)).

```text
action edited(text: string, e: InputEvent)
  send doc = edit(text, e.selectionStart, e.selectionEnd)
action jump(line: number)
  setSelectionRange("editor", line, line)
textarea id="editor" value=source input=edited select=selected
```

### Keys

`focusin` and `focusout` bubble from the element that gains or loses focus to
each ancestor handler. A wrapper with either handler gains no Tab stop. Both
offer an optional `FocusEvent`: `target` is the focused element's `id` (empty
when it has none), and `relatedTarget` is `some(id)` for the other element or
`none` when there is none. `focusout` fires even when focus moves to another
descendant of the same wrapper; compare `relatedTarget` to decide whether
focus left the subtree. `stopPropagation()` stops the current event from
reaching further ancestor handlers. Autofocus also delivers focus transitions;
IDs reflect their value when focus moves.
Web, AppKit, UIKit and Linux dispatch these transitions. An edited field sends
`change`, `blur`, `focusout`, then the next field's `focus`, `focusin`. If `blur`
removes a wrapper, its `focusout` still runs inside the detached subtree;
former connected ancestors do not hear that event, even while an exit animation
keeps the removed wrapper visible. A blur caused by a tree update runs before
the update's focus commands or autofocus. A nested focus transition
from commit processing does not cancel the outgoing event.

`key` is the DOM's `keydown` and `keyup` its `keyup`, on every host (web,
macOS, iOS and iPadOS with a hardware keyboard, Linux):

- **Where.** The key goes to the focused element: a field or textarea being
  edited, a `button`, or any element with a `press`, `focus`, `blur` or `key`
  handler (such an element takes the focus, as `tabindex="0"` gives it, and
  is in the Tab order), or with a `tabindex`. It then bubbles: the
  focused element's handler hears it first, then every ancestor's, innermost
  first. With nothing focused (at load, or after the focused element is
  removed) the path is the first root, the app's outermost node: its
  `keycapture`, `key` and `keyup` handlers hear the key after the
  `aria-keyshortcuts` buttons, as the DOM's `window` listeners hear a key
  whose target is `<body>`, and a `preventDefault()` there cancels the key's
  default (Tab's move to the first stop; the terminal's scroll). The root
  takes no focus by it: no `focus` event, no ring, no Tab stop. So on the
  web, macOS, iOS, Linux and Windows. The terminal's root hears `key` alone,
  and first, as its focused node's handler does: before the shortcuts and a
  dialog's Escape. Android hands keys only to a focused field.
  On the Mac a click on a `button`, or on a box that is only pressable, leaves
  the focus where it is, as a click on an AppKit button does: a field beside
  it keeps its caret, and its pending `change` is committed before the `press`
  runs. A click on empty ground moves no focus either. An element with a
  `tabindex`, a `button` included, or a box that is no button and has `focus`,
  `blur` or `key` handlers, still takes a click's focus there, and `focus(id)`
  moves it to any of them.
- **What.** The payload is `KeyboardEvent.key`: the character typed, Shift's
  included (`"a"`, `"A"`, `"7"`, `" "`, `"/"`), or the key's name (`"Enter"`,
  `"Escape"`, `"Tab"`, `"Backspace"`, `"Delete"`, `"ArrowUp"`…, `"Home"`,
  `"End"`, `"PageUp"`, `"PageDown"`, `"F1"`…, `"Shift"`). Every key is heard,
  printable ones in a field included. On macOS and iOS, keys an input
  method is composing are its own, but for two: a modifier's own press and
  release (so a Shift let go mid-syllable is still a keyup), and a ⌘ chord,
  whose keydown commits the composed text first: the chord's `key` handlers
  then run on committed text and read the value Chrome's would, which the
  field's `input` heard as it was composed (the commit is not heard again). A
  composer's ⌘Enter `key` action sends what was typed. On macOS such a chord
  presses no `aria-keyshortcuts` button, as the web's skip a key that came
  composing. On macOS a press outside the field (a mouse-down anywhere else
  in its window) commits the composition before the press runs, as
  Chrome's mousedown ends one, where AppKit would leave it marked: a Send
  button's action reads committed text, and no syllable stays with the
  input method once the action clears the field. On the web, Chrome hands
  the `key` handlers every key while composing, each `isComposing` (below),
  and commits nothing before a chord's handlers.
- **Modifiers.** An action that takes one more parameter, typed
  `KeyboardEvent`, hears the event too: the record `{ key: string, shiftKey:
  bool, ctrlKey: bool, altKey: bool, metaKey: bool, code: string, repeat:
  bool, isComposing: bool }`, the DOM's fields (`altKey` is Option and
  `metaKey` Command on a Mac). A key typed with Control or Meta held is a shortcut: it types
  nothing. `code` is the physical key, whatever the layout or input source
  types there (`"KeyB"`, `"Digit1"`, `"Space"`, `"MetaLeft"`), so a shortcut
  can match the key under a Korean or Russian layout; `""` where the host
  cannot tell (a software keyboard's key). `repeat` is true on the keydowns
  the platform repeats while a key is held, so an action can act once per
  press (iOS reports none: UIKit's presses carry no auto-repeat).
  `isComposing` is true on a key that comes while an input method composes
  text, between its `compositionstart` and `compositionend` (UI Events). On
  the web that is every such key, the Enter that picks a candidate
  included, so an Enter or Escape handler skips it with `not e.isComposing`,
  as web code does. A keydown an input method processed, `keyCode` 229 in
  the DOM, is composing there too, as web code's `e.keyCode === 229` test
  takes it: Safari sends the Enter that commits a composition after
  `compositionend`. On macOS and iOS the only composing keys a handler hears
  are a modifier's own press and release, which are `isComposing` as
  Chrome's are; a ⌘ chord is not, and neither are the keys after it, ⌘'s
  release included, since its keydown has committed the text (Chrome's
  stay composing).
  Linux, Windows, the terminal and Android hand handlers no composing key,
  and it is always false there.
- **Release.** `keyup` hears the key coming up, at the focus, bubbling as
  `key` does (`stopPropagation()` stops it the same way), with the same
  name and record (`repeat` is false). A modifier's release is one too
  (`key` `"Meta"`), and its own flag is no longer held (`metaKey` is true
  on Meta's keydown, false on its keyup), so an app shows a "⌘ held" hint
  between the two. A keyup reaches the focus whatever took the keydown (a
  shortcut, a `preventDefault()`). It has no default to prevent on a native
  host. An element with a `keyup` handler takes the focus, as one with
  `key` does.
- **Capture.** `keycapture` hears a keydown on the outermost ancestor first,
  down to the focused target, before the target and ancestors hear `key`.
  It takes the same name and optional `KeyboardEvent` as `key`. An ancestor's
  `stopPropagation()` during capture stops target and bubble handlers,
  including the target's `key` handler when called at the target. A capture
  stop still lets Enter submit; a capture handler's `preventDefault()`
  cancels the field edit, submission or other default.
  Putting `keycapture` on a container does not add a Tab stop.
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
  stop for Exact's key handlers: no ancestor's `key` handler hears it, and its
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

Hints while ⌘ is held, and a held ⌘W that closes one panel, not one per
repeat, by the physical key whatever the layout types. Held modifiers are
tracked as a page tracks them: every keydown and keyup says what is held,
and losing the window's focus forgets it, since a key let go while another
app or window has the focus sends no keyup, here as in Chrome. A page's
`window` `blur` listener is `exactPage().hasFocus` turning false, the gate of
a task that clears the flag:

```text
action down(k: string, e: KeyboardEvent)
  hints = e.metaKey
  if e.metaKey and e.code == "KeyW" and not e.repeat
    closed = closed + 1
    preventDefault()
action up(k: string, e: KeyboardEvent)
  hints = e.metaKey
action forget()
  hints = false
task release when hints and not page.hasFocus
  after(1, forget)
```

`resource page = exactPage() as shape Page` (`hasFocus: bool`), `column
key=down keyup=up`, and `if hints and page.hasFocus` around the hints. The
driver holds and releases a key with `type "list" key "Meta" down` and `key
"Meta" up`, takes the focus away with `prefer has-focus false` (then `clock
+1` for the task), and `key "a" for 1200` repeats a key while held, as a
keyboard does (the first repeat 500 ms after the down, then every 83 ms, on
the virtual clock).

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
  inside it hear their chords (on the Mac, an item of the app menu or Help
  still does, as below), and Enter or Space with the focus on a
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
  Go; ⌘Z ⇧⌘Z ⌘X ⌘C ⌘V ⇧⌘V ⌥⇧⌘V ⌘A ⌘D ⌘F ⌘G ⇧⌘G are Edit; ⌘= ⌘+ ⌘- ⌘0 and
  any ⌃⌘ chord are View; the rest are File. One whose chord is the host's own
  Edit item's (Undo, Redo, Cut, Copy, Paste, Select All) takes that item's
  place, so Edit ▸ Undo is the app's "Undo Move"; a paste variant (⇧⌘V,
  ⌥⇧⌘V) follows Paste, as Paste and Match Style does in TextEdit, Safari and
  Chrome ("Paste as Text"); the rest follow Select All. Any other host item
  whose chord a button declares keeps its place without the chord (File ▸
  Close Window beside the app's ⌘W). Drop the chord while a field is being
  edited and the host's text Undo, Cut, Copy and Paste come back (studio
  diary R16). Edit also holds Speech ▸ Start Speaking and Stop Speaking, which
  read the selected text aloud, in a field or out of one (#141).

  Explicit placement: `button "Check for Updates…" -exact-menu="about"
  press=checkUpdates` also becomes a menu item without a shortcut. The literal
  ids are `about` (the app menu, directly under About, as apps that update
  themselves place Check for Updates…), `app` (after About's separator, with
  Settings…), `file`, `edit`, `view`, `go`, `window`, `help`, and `none`, which
  shows no item and keeps the chord a shortcut (a host item with that chord
  keeps its place without it); they are independent of localized menu titles
  and override chord filing. The label remains the button's `aria-label` or
  text. `disabled`/inert disable the item; hidden or removed buttons leave the
  menu. An item in the app menu or Help (`about`, `app`, `help`, and Settings…)
  is the app's, not its window's: it stays enabled, and its chord works, while
  no window is key, under a sheet and under a Contract modal dialog, as in a
  hand-built Mac app; an authored `inert`, or a route behind a modal route,
  still disables it, and an app-modal panel (`NSApp.runModal`) disables it
  as AppKit disables every item then. The agent uses `tap` on that
  same button. Help retains the system search field; the standard menus, and
  Develop in a development build, remain unless removed. Other hosts keep the ordinary button
  and ignore this Mac presentation hint. On a top-level root,
  `-exact-menu-remove="show-tab-bar autofill close-all"` suppresses the named
  standard items and their menu equivalents. The literal whitespace-separated
  ids are `close-all`, `autofill`, `show-tab-bar`, `show-all-tabs`,
  `enter-full-screen`, `minimize`, `zoom`, `bring-all-to-front`,
  `show-previous-tab`, `show-next-tab`, `move-tab-to-new-window`,
  `merge-all-windows`, `back` (Go ▸ Back), `develop` (the Develop menu),
  `paste-and-match-style`, `find`, `spelling-and-grammar`, `substitutions`,
  `transformations`, `app-help` (Help ▸ *App* Help), `actual-size`, `zoom-in`
  and `zoom-out`, and `status-quit` (the host's Quit ending a status item's
  menu, LLP 1133.025 D7). A removed submenu (`find`) takes its items' chords with it,
  and an app command that would stand in there follows Select All instead.
  Empty removes nothing; duplicates are harmless. Unknown ids, expressions and
  non-root use fail with `lower-menu-remove`, which names the accepted set.
  Removing the root declaration restores the items; the policy follows the
  active window. It never removes an app button; `develop` takes the Develop
  menu off the bar and leaves its chords (Save Trace's ⌥⌘T among them);
  other hosts ignore it ([LLP 1021 §5.3](../llp/1021-menus.rfc.md#53-contract-extensions-to-the-mac-menu-bar-2026-10-12)).

  On a top-level root, `-exact-menu-keys="close-window=none"` keeps File ▸
  Close Window (built with ⌘W) in its place with no chord, so ⌘W reaches the
  page, and `close-window=Meta+Shift+W` gives it ⇧⌘W. `close-window` is the
  one id; the chord is one `aria-keyshortcuts` chord with `Meta` that is not
  one of the host's or the system's own (⌘Q, ⌘W, ⌘H, ⌘M, ⌘-comma and the like).
  Another id, an id named twice, a chord without `Meta`, a standard chord,
  expressions and non-root use fail with `lower-menu-keys`. With nothing
  holding ⌘W, AppKit's Close All ⌥⌘W shows as a row of its own: an app that
  removes ⌘W declares `-exact-menu-remove="close-all"` too. An app command
  with the same chord still takes it from the item; removing the declaration
  gives it ⌘W back. Other hosts ignore it
  ([LLP 1021 §5.3.3](../llp/1021-menus.rfc.md#533-close-windows-chord-2026-10-28)).

  The Dock menu and the status item ([LLP 1133.025](../llp/1133.025-desktop.rfc.md)
  D4, D5): `dock` beside one menu id, or alone, files the button in the Dock
  menu too, above AppKit's own items, in declaration order, from the key
  window's session. `-exact-menu="status"` stands alone (with another id it
  fails `lower-menu-status`) and presents the button as the app's
  `NSStatusItem` on macOS and its notification-area icon on Windows, taking
  its place in the page; its `popovertarget` menu is the item's menu. A click
  on macOS opens the menu when there is one and runs `press` when there is
  none; on Windows the left button runs `press` (or opens the menu with no
  `press`) and the right one opens the menu. With
  `host.macos.activationPolicy: "accessory"` the app has no Dock tile and
  opens no window at launch. Bare `focus()` is the window's, as the web's
  `window.focus()`: it shows the session's window if hidden, brings it
  forward and activates the app; `focus(id)` stays an element's.

  View holds Actual Size ⌘0, Zoom In ⌘+ (⌘= too) and Zoom Out ⌘-, as Safari's
  and Preview's View does, stepping through Safari's zoom levels (50% to 300%)
  for the main window (the key one, or the one under a key panel). The zoom
  multiplies the root font size last, over the
  app's own `setRootFontSize` as a browser's page zoom is over
  `html { font-size }`, so every `rem` and inherited text size grows; it is
  per window and not kept across a launch. Declared deviation: a browser's
  zoom scales `px` lengths too, and this one leaves them, as it leaves the
  platform's own controls at their size; an app that sizes in `px` alone
  removes the three items (`actual-size zoom-in zoom-out`). An app's own ⌘0,
  ⌘= or ⌘- command takes the host item's place.

  A held ⌘Q, as Chrome's Warn Before Quitting asks for: a root `key` handler
  that calls `preventDefault()` on ⌘Q (match `code == "KeyQ"`: under a
  non-Latin input source `key` is that source's letter) hears its press, each
  auto-repeat (`repeat`) and its `keyup`, and the menu's Quit never runs; the
  app calls `close()` to end a one-window app when the hold completes. Quit
  chosen with the pointer still quits at once, after `beforeunload`. The
  host's own items (Copy, Quit, Minimize, …) match a ⌘ chord under a
  non-Latin input source as AppKit matches them in any Mac app: the event's
  characters with ⌘ are the key's Latin letter (measured under Korean 2-Set).

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

On the Mac which elements are stops by kind follows the person's Keyboard navigation
setting (System Settings ▸ Keyboard), as in every Mac app. Off, the default, Tab moves
between text fields and what takes keys of its own (a box that is no button with
`focus`, `blur` or `key` handlers, a canvas that takes input): a `button`, a pressable
box and a radio group are passed over. On, Tab reaches every control, including
each inline link or pressable text run inside a paragraph, including in a dialog.
Return activates a focused inline link before a dialog's default button. Space
activates a pressable run only when it has no `href`; it never follows a link.
A `tabindex` you write wins either way: `0` or more is always a stop, a negative one never. A button that
`focus(id)` or `autofocus` reached while it is no stop still goes on to the next stop
with Tab, and back to the one before with Shift-Tab. Under the agent the setting is the
drive's, off unless it says `prefer keyboard-navigation true` (`state.focus.keyboardNavigation`
reads it); a `type <id> key Tab` after that walks every control.

A new Mac window puts the caret in its first stop, as AppKit does: with Keyboard
navigation off the first text field, and with no stop nothing. An `autofocus` you wrote
elsewhere wins, and a window whose first stop is in a `canvas` that takes input opens
with nothing focused. The web and iOS focus nothing on load unless `autofocus` says so.
An `autofocus` mounted later takes the focus only when nothing holds it, as in Chrome: a
`tabindex` button a tap focused keeps it when a timer remounts an `autofocus` element.
The tap's own work is the exception on the web and iOS: a field it shows, or the
`autofocus` field of a sheet, dialog or popover it opens, takes the focus from the control
tapped ([LLP 1035.000](../llp/1035.000-dependable-host-semantics.rfc.md) D9). On macOS and
iOS a click or touch whose press mounts an `autofocus` text field or textarea begins editing
it as the platform's click-to-edit does: on the Mac the field takes the focus from any field
that held it, and on both the caret goes under the press (the end below the text). The web
puts the caret where its focus does, at the end.

`aria-disabled=true` (or `aria-disabled="true"`) reports an unavailable control
to accessibility while keeping its focus and Tab behavior. It accepts a bound
boolean on a control or an element with a role, including a group. Its `press` still runs,
as on the web; guard the action in the app when it must do nothing. Descendant
controls and inline links inherit this state, including toolbar items, grouped-list
rows and content lifted into a popover; an explicit
`aria-disabled=false` on a descendant overrides it. Use HTML
`disabled=true` when the control should leave the Tab order and refuse input.

## Host commands

The current command name inventory is:

`blur`, `copyText`, `deliveryActivate`, `deliveryCheck`, `fastSeek`, `focus`, `format`,
`load`, `openURL`, `removeAllRanges`, `selectText`, `setSelectionRange`, `setRangeText`, `setScheme`, `setRootFontSize`, `showPicker`, `share`, `saveFile`,
`showOpenFilePicker`, `showDirectoryPicker`, `showSaveFilePicker`, `requestPermission`, `scrollIntoView`, `scrollBy`, `showModal`,
`haptic`, `postMessage`, `reload`, `relaunch`, `close`,
`playSound`, `playSounds`, `stopSounds`, `requestMIDIAccess`, `sendMIDI`, `startTrace_EXPERIMENTAL`, `stopTrace_EXPERIMENTAL`
([pointer](#pointer): a window's `beforeunload`), `preventDefault` ([keys](#keys)) and
`stopPropagation` ([events](#events)).

These appear only as action statements. They are not ordinary value-returning
functions. Some have dedicated compiler checks while others also rely on host
argument validation. Use the working implementation when selecting arguments:

| Operation | Starting point |
| --- | --- |
| `postMessage(text, target)`: ordered messages to a canvas by literal surface name, or string replies and host updates to a live iframe by `id` on Apple and both web executors; iframe targets may compute. Missing, ambiguous, replaced or navigation-revoked iframe targets are dropped without entering the canvas queue. | [LLP 1020 §11](../llp/1020-webview.rfc.md#11-addressed-string-replies-and-host-updates-2026-10-19), [iframe fixture](../scripts/fixtures/iframe-post-messages.contract), [compiler tests](../contract/cli/tests/it/post_message.rs) |
| `navigate(id, url)`, `traverseHistory(id, delta)`, `reload(id)` or `reload(id, ignoreCache=true)`, `stopLoading(id)`: move the mounted `iframe` with that `id`, by the Navigation API's and WebDriver BiDi's names; bare `reload()` stays the development host's reboot. The frame reports with `navigate=`, `currententrychange=`, `navigatesuccess=` and `navigateerror=`, each with a `FrameNavigation` record as the action's optional last parameter. On Apple a frame with `partition=` is a top-level page in that partition's own storage (`persist:` names last across launches) and takes them all; an unpartitioned frame refuses the commands (`navigateerror`, `NotSupportedError`). The web honours no `partition` (`exactPage().iframePartition` is false) and gives a same-origin frame what the browser lets a page see. A literal id no `iframe` carries is refused here; `partition` beside `sandbox` (`lower-iframe-partition-sandbox`), beside `message=` or a `postMessage` to it (`lower-iframe-partition-message`), or with a literal `src` that is not a web page (`lower-iframe-partition-src`) is refused. A partition is cleared with `clearSiteData` from `@exact/browser`: `send m = clearSiteData(partition=…, types=[…])`, or from data code `await clearSiteData({ partition, types })` once the Contract names the send (LLP 1133.002 D14). On Apple any frame that handles `newwindow=` asks for windows, with a `FrameNewWindow`: a `new-window` (`window.open` with features) has already opened as the platform's popup in the same partition, a `foreground-tab` or `background-tab` (`target=_blank`, ⌘-click) opened nothing and is the app's; `download=` reports a download with a `FrameDownload`. Dialogs, file inputs, a download's destination and another app's scheme are the platform's sheets and panels, and `requests` under the agent (`dialog`, `open-file`, `save-file`). `captureScreenshot(id, "app:/tmp/shot.png")` writes a PNG of any frame's guest, and `clip=DOMRect(x=…, y=…, width=…, height=…)` a part of it in the guest's CSS pixels, as a pinch or pan leaves it; the frame answers with `capture=` and a `FrameCapture` (`path`, `width`, `height`, or `error` and `message`); a literal path outside `app:/data`, `app:/cache` and `app:/tmp` is refused here (`lower-frame-capture-path`); the web answers `NotSupportedError`. On Apple an unpartitioned, unsandboxed frame of a local PDF (`app:/` or `assets/`) shows it in PDFKit's `PDFView`, `#page=N` opening at that page. Data code that holds the grant `browser.automation` drives a partitioned frame with `@exact/browser-automation` (WebDriver BiDi's `evaluate`, `callFunction`, `addPreloadScript`, `performActions`, `handleUserPrompt`, `setDownloadBehavior`, `captureScreenshot`); while it drives one, the frame's dialogs fire `userpromptopened=` with a `FrameUserPrompt` instead of showing a sheet, until a person's input takes the frame back | [LLP 1020.002](../llp/1020.002-a-browser-shell.rfc.md), [fixture](../scripts/fixtures/iframe-partition.contract), [windows](../scripts/fixtures/iframe-windows.contract) |
| `focus(id)` | [Markdown Stress](../apps/markdown-stress/app.contract) |
| `format(id, command[, argument])`: a Markdown editor's toolbar command ([Markdown](#markdown-markup-format-select)) | [Markdown Stress](../apps/markdown-stress/app.contract) |
| `blur()`, `blur(id)` | [Messages](../apps/messages/app.contract), [keyboard-bar corpus](../contract/corpus/keyboard-bar.contract) |
| `selectText(...)` | No app fixture; the hosts' field selection, such as [`FieldSelections.swift`](../host/apple/Sources/ExactKit/FieldSelections.swift) |
| `setSelectionRange(id, start, end[, direction])`: a text field's or text's selection, by its `id` ([form controls](#form-controls-radio-inputevent-setselectionrange)) | [radios conformance](../host/web-js/conformance/radios.contract), [control tests](../contract/cli/tests/it/controls.rs) |
| `setRangeText(id, text[, start, end[, selectionMode]])`: a field's range replacement, one `input`, preserve selection by default; no undo step on a plain field, one on the Markdown editor ([form controls](#form-controls-radio-inputevent-setselectionrange)) | [Chrome comparison](../host/web/tests/field-range.test.mjs) |
| `copyText(text)` | [Messages](../apps/messages/app.contract) |
| `startTrace_EXPERIMENTAL()`, `stopTrace_EXPERIMENTAL()`: experimental, so named while the trace's fields may still change; a recording of each display-link callback, from which it estimates the frames the display showed again, from an app's own control and in any build, a production one included; `stopTrace_EXPERIMENTAL()` puts the trace on the clipboard as compact JSON and in a `trace-recording-*.json` file in the app's temporary directory; `stopTrace_EXPERIMENTAL(maxBytes)` copies it cut to that many bytes (its totals and scroll segments as rows, then the frames late the longest first, as many as fit), for a conversation that limits a message (a budget too small for its totals gets a note naming the size that fits them), and still saves the whole trace. On iOS it splits each scroll a person makes of an element's scroller, a route's page or the document into its drag and its glide, with an estimate, from the display link's callbacks, of the hitch time ratio Apple's scroll metrics report; a Mac recording follows no scroller. It lists each late frame with the main thread's busy time, overrun, batches and host phases. Apple hosts (tvOS, with no clipboard, only saves the file); the web and Linux refuse it in the journal | [`TraceRecording.swift`](../host/apple/Sources/ExactKit/TraceRecording.swift) |
| `openURL(url)`: an absolute `http`, `https`, `mailto` or `tel` URL, or one of a scheme app.json's `open_url.schemes` declares (`"open_url": {"schemes": ["vscode", "zed"]}`, an editor's deep link), opened by whatever the system registered for it; the web's JS target, macOS and iOS open declared schemes (a browser asks first) and journal one nothing could open, and under the agent journal them instead (`openURL vscode: …`). A link's `href` of a declared scheme opens on macOS and iOS, which hand followed links to `openURL`; the web writes no such `href` (its link policy keeps the four schemes). `file`, `javascript`, `data` and the other schemes that run or read content cannot be declared. Any other scheme is refused in the journal ([LLP 1133.000](../llp/1133.000-the-roster.rfc.md) §2) | No Contract fixture; the hosts' dispatch, such as [`host/web-js/commands.js`](../host/web-js/commands.js), [`OpenURL.swift`](../host/apple/Sources/ExactKit/OpenURL.swift) |
| `setScheme(scheme)`: the app's scheme, `"light"`, `"dark"` or `"system"` (follow the system's appearance) | [Caltrain](../apps/caltrain/app.contract), [Markdown](../apps/markdown/app.contract) |
| `setRootFontSize(px)`, `setRootFontSize("125%")`, `setRootFontSize("medium")`: CSS's `:root { font-size }`, the root font size every `rem` follows, in px above 0, laid out in the action's own commit; `px` lengths stay. It stands over the host's own size (the browser's setting, iOS Dynamic Type, 16 on macOS and Linux), as an author's `html { font-size: 20px }` stands over a browser's font-size setting, and `"medium"` hands the size back to the host. The Mac's View ▸ Zoom In multiplies whichever size stands ([The Mac's menu bar](#keys)). A percentage (CSS's number syntax, above 0) is that share of the host's size and follows it as the host's changes (LLP 1121 D6). A literal of 0 or less is refused here, a computed one in the log when it runs. Not kept across a launch: set it again from a mount task (LLP 1069.000 D3) | [rem tests](../contract/cli/tests/it/rem.rs) |
| `share(title=, text=, url=, files=)`: the system share sheet by the Web Share API's names, at least one of `text`, `url` and `files`, each its own item of the sheet (pass a picture alone to share only it, as a Share Card button does; `contract vocab share`); `url` absolute `http(s)`. `files=` lists `app:/` files, each behind the app's `fs.read` grant (on Apple a regular file of at most 64 MB, reached through no symbolic link), and `"#id"` entries: a PNG of that element as it is drawn now, at the display's scale, named by the share's `title` (else by the id) (iOS and macOS; the web refuses it, `share: refused: a picture of an element is not available on the web`; Android and Linux refuse files). A literal `#id` no element carries is refused here (`lower-share-picture`), a missing one at run time in the journal. Offer it where `exactPage()`'s `canShareFiles` and `canSharePictures` hold (LLP 1125 D12). The outcome is a journal line | [share corpus](../contract/corpus/share.contract), [Metronome](../apps/metronome/app.contract) |
| `showPicker(id)`, export `saveFile(id, from, suggestedName)`: the host copies the `app:/` file `from` to where the person chooses; `change` at `id` carries the chosen name, `cancel` a dismissal. `saveFile(id, text=…, suggestedName=…)` saves the text itself (UTF-8), no file written first and no grant, so "export what's on screen" is one press | [picker tests](../contract/cli/tests/it/picker.rs), [Fieldnotes](../apps/fieldnotes/app.contract), [Linux's save tests](../host/linux/src/presenter/save_tests.rs) |
| `showOpenFilePicker(id[, multiple])` | [file-picker corpus](../contract/corpus/file-pickers.contract) |
| `showDirectoryPicker(id)` | Same corpus |
| `showSaveFilePicker(id, suggestedName)` | Same corpus |
| `requestPermission(id, path, mode)`: a kept document's access asked for again, `mode` `"read"` or `"readwrite"`, from a press (the web asks the browser under its activation; a native host answers `granted` for a document that resolves); `change` at `id` hears `granted` or `denied` ([documents kept across launches](reference.md#documents-the-person-chose-doc)) | No app fixture; [the web's kept documents test](../host/web/documents-kept.test.mjs) |
| `scrollIntoView(id, block=, inline=, behavior=)`: `Element.scrollIntoView()` on any element by its `id` (a string, dynamic as `focus`'s): every scroll container above it, innermost first, then the page, align it by the web's `ScrollIntoViewOptions` (`block` default `start`, `inline` `nearest`). `scrollIntoView("list-id", key, …, row=, offset=)`: a virtualized list's row by key, built and measured first (LLP 1070.000). macOS, iOS and tvOS animate the element form with the platform's scroll animation unless `behavior="instant"` (LLP 1115.000 D8.4), landing immediately under reduced motion; `scrollend` comes once the animation ends. Linux lands element jumps at once, including smooth requests (LLP 1070.000 §14) | [collection tests](../contract/cli/tests/it/collection_into_view.rs) |
| `scrollBy(id, x, y)`: `Element.scrollBy(x, y)` on a scroll container by its `id`, in pixels (a terminal's row is 16, its column 8), clamped to its travel; an element that does not scroll does not move. A reader's `j` and `k`. The terminal, Linux and the web's JS target take it; Apple and the wasm web host do not yet | [the LLP terminal reader](../apps/llp/terminal.contract), [`reading.rs`](../host/terminal/tests/it/reading.rs) |
| `showPopover(id, source="trigger")`, `hidePopover(id)`, `togglePopover(id, source="trigger")`: HTML's methods by element id. `popover=(expanded ? some("manual") : none)` adds or removes that property on the same node; removing it closes the top layer and restores inline placement. `source=` is optional and explicit; it need not carry `popovertarget`, and the key handler's node is not inferred. Repeated show/hide is a no-op. Visibility changes immediately in command order, including hide then show during native dismissal. Bad targets or sources are refused in the journal. A popover's `toggle=changed` action may take `e: ToggleEvent`, with `oldState` and `newState` (`"open"` or `"closed"`); events follow logical visibility, coalescing changes in one turn. Without a source the popover has no invoker. A following `focus(id)` in the opening action selects a native menu row on macOS and windowed Linux; AppKit refuses changing the highlight once native tracking begins. UIKit navigation-bar menus still lack command dismissal and toggle observation (issue #319 remaining work). Desktop/headless Linux reports unsupported; windowed Linux uses its existing supported menu/alert shapes. | [menus](../llp/1021-menus.rfc.md) |
| `showModal(id)`, `close(id)`: `HTMLDialogElement.showModal()` and `close()` on a dialog by its `id`, from an action (bare `close()` is the window's). The terminal takes them (LLP 1101.001 P5); iOS presents a `dialog` or `role="alertdialog"` popover by its id as its confirmation (an alert or action sheet, with no invoker) and any other `dialog` as a sheet as tall as its content (LLP 1115.000 D11.5), the web shows the dialog (or the alertdialog popover), macOS the dialog in its top layer, or a `role="alertdialog"` one in the alert's shape as `NSAlert`, a sheet on the window (LLP 1115 D6, LLP 1115.000 D7.6) | [the harness](../apps/harness/terminal.contract), [the native fixture](../apps/native-fixture/app.contract) (`askClear`) |
| `fastSeek(id, seconds)`, `load(id)`: a `video` or `audio`, by HTML's method names (LLP 1042 §3). `fastSeek` seeks each time it runs, where a bound `currentTime` seeks only when its value changes; every host seeks to the exact time, which HTML's approximate-for-speed allows. `load` loads the source again, as a changed `src` does: the bound `currentTime` waits for its metadata and a bound `paused` false plays | [media tests](../contract/cli/tests/it/media.rs), [media conformance plan](../host/web-js/conformance/media.contract) |
| `deliveryCheck`, `deliveryActivate` | [delivery corpus](../contract/corpus/delivery.contract) |
| `playSound(src, at=, gain=, group=, playbackRate=, pan=)`: a new voice of a declared sound (a literal `src` must be declared, `type-sound-undeclared`), starting at `at` on the runner's clock (`performanceNow()`'s milliseconds; the past and the default are the commit's time), at a linear `gain` 0–1 (default 1; a literal outside is refused, a computed one clamped), in a `group` that is monophonic by start time, read at `playbackRate` 0.25–4 (default 1; pitch and length move together, so the voice lasts its file's length divided by it) and placed at `pan` −1 to 1 (default 0, centred; Web Audio's equal-power `StereoPannerNode`), each refused as a literal outside its range and clamped when computed, as `gain` is (LLP 1125 D10). `playSounds(hits)`: one voice per item of a list of a shape whose fields are, in order, `src: string`, `at: number`, `gain: number`, `group: string`, then optionally `playbackRate: number` and then `pan: number`. `stopSounds()`, `stopSounds(group=)`: what sounds stops, what waits is cancelled. The runner keeps the voice table (`state sounds`); the web and Apple play it, Linux and Windows keep the record (LLP 1096) | [sound tests](../contract/cli/tests/it/sound.rs), [sounds conformance](../host/web-js/conformance/sounds/app.contract) |
| `requestMIDIAccess()`, `sendMIDI(output, data, at=)`: ask for the MIDI ports `exactMIDI()` answers; send whole MIDI 1.0 messages to an output by `id` at `at` on the runner's clock. The runner keeps the queue (`state midi`); a host hands each message to its output 50 ms before it is due ([MIDI](#midi-exactmidi-sendmidi-midimessage), LLP 1125 D6, D7) | [MIDI tests](../contract/cli/tests/it/midi.rs), [MIDI conformance](../host/web-js/conformance/midi/app.contract), [Metronome](../apps/metronome/app.contract) |

The web (its JS target) and the Apple hosts carry every command. The
headless Linux host has no browser, clipboard, editor or dev menu: its
`openURL`, `copyText`, `format`, `reload`, `close`, `fastSeek` and `load` are
journaled as unsupported there (it has no media player), and `haptic` does nothing. Its `selectText` focuses the field
with its whole text selected, which the next typed key or Backspace replaces.
On Android the same host has the platform behind it (LLP 1115.001 D5): `openURL` and a
link out open the browser, the mail app or the dialer, `copyText` writes the clipboard,
`haptic` and `-exact-press-haptic` play, and `share` shows the Sharesheet; notifications
and the pickers are still refused there.
iOS closes no window either: its `close` is journaled as unsupported. On the web,
`reload()` is the page's own reload, and `deliveryCheck` and
`deliveryActivate` find nothing (a web build has no update store; the page is
the newest root).

`relaunch()` takes no arguments. In the standalone macOS app it asks the same
`beforeunload` handlers as Quit, waits for pending storage and each native
module's quit work (`willQuit`) for up to five seconds in all, destroys the sessions, then starts the same executable with the same arguments
and environment after the old process exits. A cancelled quit cancels the relaunch.
It works without the development menu. iOS/tvOS and Linux refuse it in the journal;
the web reloads the page, with the browser's usual storage and unload rules.
Native agent sessions refuse it explicitly: start another drive on the same
`--storage` instead. `reload()` and update activation keep their existing lifecycles.

Element-targeted commands use `id`, not `testId`. File pickers publish handles
through the target's `change` event; cancellation uses `cancel`. Permissions,
accepted file types, app identity, and platform support remain part of the app
integration. A recognized command name alone does not prove those prerequisites.

## Semantic restrictions

Syntax is only the first layer. In particular:

- Types are closed; unknown/unresolved types, cycles, and invalid property values
  are refused. `none` and `[]` need enough context to infer their contained type,
  or a state's declared one (`state ids: string[] = []`).
- Record construction requires all fields once; copying requires a matching base.
  Shapes are finite and nonrecursive. Equality compares values structurally.
- Function bodies are effect-free, nonrecursive expressions (they may read
  `performanceNow()`); an app's function may shadow a roster name (and is then
  called `name(x)`, as it is declared) but not take a declared shape's constructor name.
- Action reads see the starting snapshot. Writes land together. Locals are
  immutable and cannot shadow visible names or escape their block.
- Resources/mutations/tasks are root-owned, but a child may read the device's
  facts (`exactViewport()`, `exactPage()`, no arguments). A child may own
  ordinary state, derives, and actions. An action writes only its own admitted slots.
- A `then` handler takes no parameters and must not send its own mutation.
- A state's initializer reads only props, injects and the states above it; a
  resource, derive, mutation, action or later state it names is
  `type-initializer-scope` (LLP 1088 D4).
- `persist` (LLP 1116 D5) keeps a root state across launches under its name: a
  stored value of its type replaces the initializer's before the first frame,
  and after each commit that stands the store holds the state's value. Only a
  number, string or bool, or an option or list of one, at most 16 KB as JSON;
  a record, a list of records or a state holding an action is
  `type-persist-type` (keep app data in a source), a child's state
  `type-persist-child`, a derive `syntax-persist-derive`.
- An action sends one mutation at most once on any path: a second send forgets
  the first's reply (LLP 1016 D5), so it is `analyze-send-twice` — unless the
  mutation is `queue`, whose sends each wait their turn (LLP 1092 D6). Exclusive
  `if`/`match` arms, and sequential `if`s testing one unchanged name against
  different literals, are separate paths (LLP 1088 D8). The walk reads the root's
  actions with every call expanded, where a caller and its callees are one commit.
- A read of a state or mutation slot that the action assigned earlier on the
  same path sees the starting value, since writes land when the action ends: it
  is the warning `analyze-read-after-assign` (LLP 1116.003 D6; LLP 1035.005.002
  C2), in the action's own body or behind a call (the action's body, or another
  call, assigned it; LLP 1089 D3), one rule at one severity. The message names
  the repair: bind the value, `let next = …`, assign `next`, read or pass `next`;
  a `let` bound before the write keeps the old value. Not named: a read on a
  path exclusive with the assignment's, a condition the assignment's path
  decides (`if phase == 1` … `phase = 2`, then `if phase == 2`), and a field a
  record copy left unchanged (`s = S(s, f=…)`, then `s.g`). A derive, a resource
  and `pending(m)` read settled values. In 1,029 bake-off builds the body's read
  was met in 86 and the call's in 2, which is why the call's is no longer a
  refusal. `pending`/`failed` operate on declarations, not arbitrary values.
- An action that both sends a nonqueued mutation and assigns it on one path, in
  either order and through calls, is warned of (`analyze-send-assigned`, LLP
  1035.005.002 C1): a send is asked when the action runs; once the writes land,
  a request still in flight for a mutation the action assigned is forgotten and
  its reply dropped, and the slot keeps the assigned value (LLP 1016 D5). The
  message names the three intents' spellings; a `queue` mutation's assignment
  drops no queued work (LLP 1092 D4) and is not named. No bake-off build wrote
  the combination; Weird Castle's logout means it.
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
