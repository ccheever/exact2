# LLP 1101: Terminal apps — a terminal host for apps authored for the terminal

**Type:** RFC
**Status:** Accepted (Charlie, 2026-10-06: "this is pretty great. commit and push this and document that TUI is now an officially supported platform for Exact2"). The terminal is an officially supported surface; `rules/DEFERRED.md` §Surfaces records the admission and §Authoring models carries D9's sentence. r1–r4 were exploratory (2026-10-05); what was built and measured since is LLP 1101.000 (the todo fixture) and LLP 1101.001 (the coding harness, whose r2 revises D3's cell, D7's writer, D10's inline mode and §4's border rule as built).
**Systems:** A new host (`host/terminal`, `exact-terminal`: a painter host whose backend is a grid of character cells), Contract (a `terminal` compile profile and the `exact:terminal` module), the schema (`kernel/tables/schema.json` gains a per-row terminal admission), the kernel (the `ch` and `lh` units), the app manifest (`app.json` `host.terminal`), the agent API (`scripts/agent.mjs terminal`; no new operation), `rules/DEFERRED.md` (§Tooling "no TUI host"; §Authoring models)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Implementer:** none yet. This document proposes; it specifies nothing until it has one.
**Related:** LLP 1015 (the Linux host: the painter host this one repeats with a cell backend), LLP 1001 §5–6 (layout is a host call; text measurement is injected), LLP 1012 / LLP 1079 (the ten operations), LLP 1023 D10 (one plan; per-platform sources, never; §7 answers it), LLP 1091 (Contract modules; `exact:` built-ins), LLP 1086 (`exact new`), LLP 1036.001 (apps outside the repo), `rules/DEFERRED.md` §Surfaces (2026-10-03: a platform with its own UI stack is a host outside the repo). Research, never authority: exact1's LLP 0368 (`~/projects/exact/llp/0368-tui-target-platform-revival.rfc.md`, the TUI target r14, which this document mostly agrees with), LLP 0507 §11.4 and LLP 0505 (why exact1's TUI host was frozen), LLP 0552 (the "TUI estate", listed for deletion).

## Summary

An Exact app can have a **terminal entry**: a second root `.contract` file, written
for a terminal from the first line, compiled under a `terminal` profile into its
own plan. It runs in a new host that paints character cells. The entry shares
everything below the view with the app's other surfaces: the data module,
resources, storage, strings, routes and `.contract` libraries. It shares no
visual structure. There is no automatic projection of a web or phone UI into a
terminal.

The vocabulary doesn't fork. A terminal entry uses the same tags and CSS
properties as every other entry, with lengths in CSS's own cell units, `ch`
(one column) and `lh` (one row). A per-row admission table refuses at bake
whatever a terminal cannot honestly draw, such as `px` lengths, shadows,
gradients, font sizes and transforms. A refused row is an error you see while
writing, never a silent degradation you discover when the app runs. Modern
terminals supply much of the rest: 24-bit colour, bold, italic, styled
underlines, SGR mouse with hover, the kitty keyboard protocol, images through
the kitty, iTerm2 and sixel protocols, OSC 8 hyperlinks, synchronized output,
and a background colour to infer dark mode from. Each is negotiated at launch,
exposed to the app as a web-named fact, and overridable by `prefer`, so an
agent can drive the 16-colour, no-mouse, no-image case from any terminal.

The host is LLP 1015's design with a different backend. The runner and kernel
run natively, Taffy lays the tree out, and the painter walks the kernel tree
into a cell grid that a diffing writer turns into escape sequences. The kernel
is the display list, and the host keeps no mirror. That property removes by
construction the bug that froze exact1's TUI host.

This is exploration, not a commitment (Status). If it is ever built, it needs a
consumer, and it moves "no TUI host" off `rules/DEFERRED.md`. §9 records the
consumers Charlie named, a todo list and the LLP reader, and what is still his
to decide.

## 1. Why now, and why this shape

**The terminal changed.** In 2015 a terminal UI meant 16 colours, no mouse
worth using, and text only. Today the terminals people actually use (Ghostty,
kitty, WezTerm, iTerm2, Windows Terminal, foot, VS Code's) mostly support
truecolor, SGR mouse reporting, bracketed paste, OSC 8 links and synchronized
output, and several show real images. Support differs by terminal, so §5
negotiates each capability rather than assuming it. The medium now covers
enough of what Contract expresses that a native-feeling app is possible, and
the people building with Exact, agents included, spend their day in it.

**Projection was tried and failed, twice.** exact1's LLP 0368 r1–r13 lowered
ordinary Contract surfaces to cells through a density profile with a
linear-layout fallback. The calibration surfaces scored 1.25–1.5 against a 2.0
floor at 40 columns. Charlie judged them unusable, and the RFC concluded that
the failure "refutes automatic visual lowering as the product," not the
terminal host (0368, Summary). Charlie saw the same thing again with exact2 web
code pushed into a terminal (2026-10-05): "you would have to basically build an
entire UI knowing it was the terminal, not trying to generalize." That
experience is the premise of this document. **D1 is not a hypothesis to test
again.**

**The map is still decent.** A terminal is a grid with a box model.
Flexbox and grid lay it out well (Ink, the React renderer behind several modern
terminal apps including Claude Code, is Yoga over cells). Text, colour, weight,
underline, borders, scrolling, focus, buttons, inputs, lists, dialogs and links
all have direct terminal forms. What breaks is *visual structure designed for
pixels*: 16-px gutters, card shadows, 13-px captions beside 30-px numerals,
rounded images. That is why the unit is the entry and not the row.

## 2. Decisions

| # | Decision |
|---|---|
| D1 | A terminal UI is an explicit entry, never a projection of another surface |
| D2 | Same language, same vocabulary; a terminal admission profile refuses at bake |
| D3 | Lengths are cells, spelled `ch` and `lh`; `px` is refused |
| D4 | The CSS-to-cell mapping (§4) is the admission table's content |
| D5 | Terminal capabilities are negotiated and exposed as web-named facts |
| D6 | Input: keyboard first, the mouse when present, focus always visible |
| D7 | A painter host: the kernel is the display list; one diffing writer; app text never reaches the terminal as control |
| D8 | The agent API: the ten operations, answered in cells |
| D9 | In-tree `host/terminal`, outside `default-members`; if built, a DEFERRED change with a consumer and a take |
| D10 | Full-screen first; inline and print-once are later forms of the same host |

### D1 — An explicit entry

`app.json` names the entry:

```json
"host": {
  "terminal": {
    "entry": "terminal.contract",
    "minimumColumns": 60,
    "minimumRows": 16
  }
}
```

The entry is a root file like `app.contract`. It is compiled with the
`terminal` profile into its own plan, with a header that says so. The terminal
host refuses a plan without that header, and every other host refuses a plan
with it (the existing header gates, LLP 1023 D10, fail closed at decode). No
host ever falls back from one to the other. An app may have only a terminal
entry: a terminal-only tool is an app whose `app.json` has only `host.terminal`.

`minimumColumns`/`minimumRows` are admission hints. Below them the host shows
its own bounded "make the window larger (60×16)" screen. It never crashes, and
it never lays the app out into a box that cannot hold it.

What the entry shares is what sits below the view: the data module and its
resources (`resource board = board(...)` reads the same Rust or TypeScript
function), storage (the same SQLite file and app-scoped files, under the same
app id), strings, routes, and `.contract` libraries through `use` (LLP 1091).
Shared `fn`s and shapes work unchanged. Shared *components* work only if they
pass the terminal profile, which in practice means components written for it.

### D2 — One vocabulary, one admission profile

exact1 gave the terminal its own component package, importable only from a TUI
entry (0368 §TUI-specific Contract). exact2 doesn't need one, because its
vocabulary is HTML and CSS: `view`, `row`, `text`, `button`, `input`, `ul`,
`dialog`, `a`, `img`, `padding`, `border`, `color`, `font-weight`,
`text-overflow`. A terminal draws most of those directly. Inventing
`Panel`/`List` primitives would fork the vocabulary that LLP 1023 D10 says must
never fork.

So each row in `kernel/tables/schema.json` gains a `terminal` column, the
admission authority beside every other declaration there:

- `admit`: drawn as CSS says.
- `cell`: admitted with lengths restricted to D3's units.
- `quantize`: admitted, approximated by the host to the negotiated capability
  (colour depth, underline styles), with the approximation declared in §4.
- `refuse`: a bake error naming the row and why the terminal can't draw it.

The terminal profile reads that column and nothing else. A row with no entry is
refused. (r4, from the spike: the column is a `terminal` key on the schema's
style rows, read by the compiler alone, so it generates no code. `refuse` is
spelled by absence. The spike marks 75 of 193 rows, §11.) Admission is opt-in per row, and that is the difference from exact1's
r1–r13, which lowered everything and scored the wreckage.

What the terminal genuinely adds is composition, not new primitives: an
`exact:terminal` built-in module (LLP 1091 D8) of components written in
ordinary Contract over admitted rows. Examples are a bordered `Panel` with a
title in its top edge, a `StatusBar` with left and right regions and
priority-aware truncation, a `KeyHelp` that lists the current
`aria-keyshortcuts`, and a `Log` that follows its tail. These are conveniences
that an author could write by hand, and the module ships only what a consumer
actually uses.

### D3 — Cells are `ch` and `lh`

CSS already has the units: `1ch` is the advance of "0" and `1lh` is the
computed line height. In a monospace font with a fixed line height, `1ch × 1lh`
*is* one terminal cell. So a terminal entry writes `padding="0 1ch"`,
`width="24ch"`, `height="1lh"`, `gap="1lh"`. Percentages, `fr`, `auto`,
`min-content`/`max-content`, flex grow/shrink and unitless `0` are admitted.
Anything that resolves to device pixels is refused: `px` (including Contract's
unitless numbers, which mean px), `pt`, `em`/`rem` (no font size to scale
from), and viewport units.

This keeps the web the standard (CLAUDE.md, "The web is the standard"): a
terminal plan is a legal web page. Rendered in a browser in a monospace font at
`line-height: 1lh`, it lays out on the same grid, which makes the web host a
possible layout oracle for this host (§5).

**Layout space stays isotropic, at a fixed cell.** One `ch` is 8 layout pixels
and one `lh` is 16, whatever the physical terminal's cell (revised r4 from the
spike, §11; r1 had the host report its real cell size to the kernel). A
constant resolves the same way at bake and at run time with no host input, and
a 1:2 cell keeps `aspect-ratio` close to square. Only an image needs the
terminal's real cell size (`CSI 16 t`, or `TIOCGWINSZ`'s pixel fields), and only
to choose the pixels it sends. Taffy lays out in that space, and the painter
snaps every box to the grid. Lengths in `ch`/`lh` land
exactly on cells. Percentages and flex distribution produce fractions, so the
snap uses one deterministic tiling rule: round each edge to the nearest cell
boundary, measured from the parent's snapped origin. Siblings then tile their
parent without gaps or overlaps, which is the rule exact1 found it needed
(0368 §Cells are the authored unit). exact1 instead made one layout unit one
cell on each axis. That is simpler, but it distorts every aspect ratio by the
cell's 1:2, and it gives unitless numbers a meaning the web doesn't share.

**Text measurement** is injected (LLP 1001 §6). It returns whole columns ×
rows from a pinned grapheme and width policy: UAX #29 clusters, East Asian
wide = 2, emoji presentation = 2, ambiguous = 1, line breaks by UAX #14. The
kernel owns wrapping, and the painter never rewraps. Where a terminal disagrees
about a cluster's width, no cursor movement can repair that, so the host
reports it rather than hiding it. Where the terminal supports grapheme
clustering mode (DEC mode 2027), the host turns it on.

### D4 — The mapping

§4 is the table. It is D4's content and the first draft of the schema column.

### D5 — Capabilities as web facts

At launch the host negotiates what the terminal can do. It queries DA1,
`XTVERSION`, DECRQM for the modes it wants, the kitty graphics and keyboard
protocols, the cell size, and the background colour through OSC 11, and it
reads `COLORTERM`/`TERM_PROGRAM` only where a query can't answer. Every answer
it exposes to the app goes through a name the web already has:

| Terminal fact | Exposed as | Notes |
|---|---|---|
| truecolor / 256 / 16 / none | `color` media feature (bits per channel); `monochrome` | The host quantizes authored colours; the app may branch to pick a better palette |
| mouse tracking available | `pointer: fine` / `none`; `hover: hover` / `none` | No mouse → keyboard only, which an app must already support (D6) |
| background luminance (OSC 11) | `prefers-color-scheme` | `light-dark()` works |
| image protocol | none (capability, not a fact) | `img` falls back by itself (§4); an app need not branch |
| size | `exactViewport` width/height, plus `columns`/`rows` in the terminal profile | `columns`/`rows` exist only in a terminal plan; the bake refuses them elsewhere |

`prefer` sets any of these (LLP 1061 D5: a new fact is a form of `prefer`), so
`agent.mjs terminal prefer color=4 pointer=none images=none` drives the poorest
case from Ghostty. This is how the fallbacks get tested at all.

### D6 — Input and focus

- **Keyboard first.** Every pressable and editable node is reachable by
  Tab/Shift-Tab in document order. Arrow keys belong to the focused list,
  scroll or field. `aria-keyshortcuts` binds keys, as it already does on every
  host (`docs/contract-for-agents.md`, "`"r"` reaches an `aria-keyshortcuts="r"`
  button"). The bake refuses two shortcuts with the same key in the same scope,
  because in a terminal the keyboard is the whole interface. When the terminal
  supports the kitty keyboard protocol, the host enables it: it disambiguates
  Esc from Alt, Tab from Ctrl-I, and reports key releases. Without it, the
  legacy ambiguities are declared, not hidden.
- **The mouse when present.** SGR (1006) press, release, drag (1002) and
  wheel, with any-motion tracking (1003) only while some node has a hover
  handler or `:hover` style. A press hits the node whose snapped box contains
  the cell, using the same hit-testing the painter's boxes give the Linux host.
  A wheel is a `tap` form (DEFERRED §Agent API).
- **Focus is always visible.** If the author styles `:focus-visible`, that
  style wins. Otherwise the UA indicator is reverse video over the focused
  node's cells. This is a declared deviation from CSS's outline, because an
  outline in a cell grid would have to draw over a neighbour's cells.
- **Text.** Fields are host-drawn, with the terminal's own cursor placed at the
  caret (its shape set by DECSCUSR) and the selection in reverse video. The
  terminal does IME composition itself, so committed text arrives as input.
  Bracketed paste (2004) goes to the focused field only.
- **Resize** (SIGWINCH) re-answers `exactViewport` and lays out once. Focus
  stays on the same node, or else moves to the nearest focusable one.
- **Exit.** `Ctrl-C` reaches the app's key handlers first. If none handles it,
  the host exits cleanly. `Ctrl-\` always exits and cannot be shadowed.
  `Ctrl-Z` suspends: the host restores the terminal, stops, and repaints in full
  on `SIGCONT`.

### D7 — A painter host

Per frame, the host:

1. settles the runner,
2. lays out with the kernel,
3. walks the kernel tree into a cell grid, where each cell holds a grapheme
   lead or continuation, foreground, background, attributes, a link id, and
   the owning node for hit-testing,
4. diffs the grid against the last one written,
5. writes the difference as one buffered write inside a synchronized update
   (mode 2026) where the terminal supports it.

There is no batch, no mirror and no retained view tree: the kernel is the
display list, as on Linux (LLP 1015 Summary). That is the structural fix for
exact1's freeze, where the host mutated its mirrors before kernel apply and
rejected sidecars after commit (0507 §11.4). Without a mirror, the ordering
mistake has nowhere to happen.

Frames are demand-driven. A commit, a resize, input, or a running timer that
changes the tree schedules one frame, coalesced to at most 60 per second. An
idle app writes nothing.

**App text never reaches the terminal as control.** Every string the app shows
comes from data the app doesn't fully control (a train headsign, a note, a
fetched title), and an escape sequence in it could rewrite the clipboard
(OSC 52), retitle the window, or spoof the screen. So the writer emits only
sequences it constructs itself. C0/C1 controls and ESC in app text are drawn as
visible replacement glyphs, and hyperlink targets are validated URLs. This is a
test, not a convention: the smoke feeds an app hostile strings and checks the
byte stream.

**The terminal is always restored.** Raw mode, the alternate screen, mouse
reporting and the keyboard protocol are undone on normal exit, on panic (a
hook), and on SIGINT/SIGTERM/SIGHUP. SIGKILL cannot be caught. exact1 ran a
separate guardian process for that case (0368 §Runtime), and this RFC doesn't:
one process, with `reset` documented as the cure. Q5 asks whether that's
enough.

### D8 — The agent API, in cells

No new operation. `scripts/agent.mjs terminal …` drives a **headless** host:
the same binary with no TTY, painting into a grid of a given size
(`--size 100x30`), with the agent API over stdio, as the Linux host does
headless.

- `tree` and `state` are unchanged. `layout` reports boxes in cells (and in
  the host's pixels, labelled).
- `tap` presses a node: a mouse press at its centre cell, or Enter/Space on it
  when `prefer pointer=none`. `type` is unchanged, keys in Playwright's
  spelling.
- `screenshot out.png` rasterizes the grid with a bundled monospace face
  through `exact-raster`, so a person or a model can look at it.
  `screenshot out.txt` writes the grid as plain text, and `screenshot out.ans`
  writes it as text with SGR attributes. These are forms of `screenshot`
  chosen by extension, as `.apng` already is. The text forms are the cheap
  diffable golden.
- `clock`, `logs`, `perf` and `prefer` (D5) are unchanged.

A human-run interactive session can also be addressed, over a socket named on
the command line, so an agent can watch what a person is doing in their
terminal.

### D9 — Where it lives, and what DEFERRED says

**In-tree, as `host/terminal` (`exact-terminal`), outside `default-members`.**
The async lane builds and tests it, like the other hosts. The 2026-10-03 rule
that sends a platform with its own UI stack out of tree doesn't fit, because a
terminal has no UI stack: it is a painter host, nearer to Linux than to Vega
OS. Also, half the work is in-tree no matter where the host lives: the schema
column, the compile profile, the `ch`/`lh` units and the agent driver.

It depends on the kernel, runner, plan, data and route crates, and on a
terminal I/O layer of three parts (Charlie, 2026-10-05: "rec seems
reasonable"):

- **Output is ours.** SGR, cursor movement, the synchronized-update brackets,
  OSC 8, the kitty, iTerm2 and sixel image encoders, and the cell diff. That
  is a few hundred lines, and the writer must construct every byte itself for
  D7's escaping to be a property rather than an audit of someone else's code.
- **Input is parsed by `vte`**, Alacritty's escape-sequence state machine. It
  turns bytes into "CSI with these parameters" or "OSC with this payload" and
  has no opinions beyond that. The host maps those to keys (the kitty protocol
  and the legacy fallbacks), SGR mouse events, bracketed paste and, the reason
  for choosing it, the replies to D5's capability queries, which arrive mixed
  in with keystrokes. That is about 500 lines of mapping, and the host sees
  every reply the terminal sends.
- **`rustix`** for termios (raw mode), signals and `TIOCGWINSZ`.

That keeps it pure Rust, light on dependencies, and buildable on any Unix with
no system packages. The Linux host's
executor (requests over rustls) and its orchestration are extracted into a
shared crate only where code would otherwise be copied, and only when that
copy actually exists. Windows (ConPTY and Windows Terminal's VT support) is
later.

**DEFERRED would change in two places, both Charlie's to make, and neither
is made by this exploratory draft:**

- §Tooling, "no TUI host" comes off, with a line naming what it unblocks and a
  take (or a waiver), per "Moving something off this list". §9 below.
- §Authoring models bans platform-suffixed overrides ("One route, one file"),
  and LLP 1023 D10 says "per-platform sources, never". A terminal entry is a
  second root file, so it needs to be argued for, not slipped past those rules.
  The argument: those rules prevent *one UI* from forking per platform until
  the variants disagree, and they protect a URL that names one plan. A terminal
  entry is not a variant of the GUI. It shares no visual structure and has no
  URL, and it fails closed against every GUI host. The GUI surfaces still never
  fork. The proposed wording, added under §Authoring models: *"A terminal entry
  (LLP 1101) is a separate application root on a separate medium, not a
  platform override; GUI surfaces still have one source."*
  **Rationale accepted (Charlie, 2026-10-05: "that rationale sounds right").**
  The wording goes into DEFERRED only if the host is built.

### D10 — Full-screen first

v1 is full-screen: the alternate screen, the app owns every cell, and the
user's scrollback comes back on exit. Two later forms use the same host and
plan:

- **Inline**, as Ink and Claude Code render: the app draws a live region at
  the bottom of the normal screen, and content it marks finished scrolls up into
  the terminal's own scrollback. This needs one new idea, a node whose rows,
  once settled, leave the live region. That idea isn't designed here (Q8).
- **Print-once**: when stdout is not a TTY, or with `--print`, the host settles
  the app, renders one frame at `--columns` (default 80) as plain or SGR text,
  and exits. This is the bridge to the command line: `todo /active --print`
  is a script-friendly list of what's left to do. The first
  argument is already read as the location (LLP 1015 §1, `exact_route`), so
  routes work unchanged.

A plain CLI with flags and no view is out of scope. That is the data module
invoked with arguments, a different program shape from anything Contract
describes.

## 3. A sketch

What the todo list's terminal entry might look like (§9 names it as a
consumer). The syntax is illustrative. `items`, `addItem`, `toggleItem` and
`removeItem` stand for the app's data module, which its web and phone entries
would share. The list, the input, the keys and the storage are all things a
terminal draws well.

```contract
use Panel, StatusBar, KeyHelp from "exact:terminal"
use Item from "./shared.contract"

component TodoTerminal
  state filter = "all"
  state draft = ""
  resource items = items(filter) as shape list<Item>

  view
    main width="100%" height="100%" display="flex" flex-direction="column" color=light-dark("#1a1a1a", "#d8d8d8")
      row height="1lh" padding="0 1ch" background-color="#1e3a5f" color="#ffffff"
        text "Todo" font-weight=700
        text `${count(items)} items` margin-left="auto"
      row gap="2ch" padding="1lh 1ch 0"
        button press=showAll aria-keyshortcuts="1" aria-pressed=(filter == "all") testId="filter-all"
          text "1 All"
        button press=showActive aria-keyshortcuts="2" aria-pressed=(filter == "active") testId="filter-active"
          text "2 Active"
        button press=showDone aria-keyshortcuts="3" aria-pressed=(filter == "done") testId="filter-done"
          text "3 Done"
      Panel(title="Tasks", flex-grow=1, margin="1lh 1ch 0")
        scroll height="100%"
          each item in items key=item.id
            button press=toggleItem(item.id) testId=`item-${item.id}` display="flex" gap="1ch"
              text (item.done ? "[x]" : "[ ]") color=(item.done ? "#5fa85f" : "inherit")
              text item.title flex-grow=1 text-overflow="ellipsis" white-space="nowrap" text-decoration-line=(item.done ? "line-through" : "none") font-weight=(item.done ? 300 : 400)
      row height="1lh" padding="0 1ch" gap="1ch"
        text "+" font-weight=700
        input value=draft change=setDraft submit=addItem placeholder="New task" aria-keyshortcuts="n" flex-grow=1 testId="new-item"
      StatusBar
        KeyHelp
```

The phone's checkbox rows become `[x]` and `[ ]` columns. The filter
segmented control becomes three keyed buttons. A completed task is faint and
struck through instead of grey, and `n` jumps to the input. That is the
terminal UI as a person would design it, written in the same vocabulary over
the same data.

## 4. The mapping (D4; the schema column's first draft)

| CSS | Terminal | Column |
|---|---|---|
| `display` block / flex / grid / none / contents | Taffy, as everywhere | admit |
| lengths, `gap`, `padding`, `margin`, `width`…, `inset` | D3 units only | cell |
| `color`, `background-color`, `light-dark()`, `currentcolor` | SGR 38/48; truecolor → 256 → 16 → none by negotiation; alpha composited over the painted background in the host | quantize |
| `font-weight` ≥ 600 / ≤ 300 | bold (SGR 1) / faint (SGR 2) | quantize |
| `font-style: italic` | SGR 3 | admit |
| `text-decoration-line` underline / line-through | SGR 4 / 9 | admit |
| `text-decoration-style` double / dotted / dashed / wavy; `text-decoration-color` | SGR 4:2…4:5, 58, where supported; otherwise a single underline in the text colour | quantize |
| `font-size`, `font-family`, `letter-spacing`, `line-height` ≠ `1lh` | the terminal's face and grid are the user's | refuse |
| `text-align`, `white-space`, `overflow-wrap`, `text-overflow: ellipsis`, `line-clamp` | on whole cells; `…` is one column | admit |
| `border-style` solid / double / dashed / dotted; `border-width` thin / medium | one cell per side. Light `─│`, double `═║`, dashed `┄┆`, dotted `┈┊`; `medium` = heavy `━┃`. CSS's `thick` and px widths are refused. The cell is reserved by the kernel itself: under a process-wide terminal flag only the terminal host sets, a drawn side occupies a row (top, bottom) or a column (sides) in layout (r4, §11) | cell |
| `border-radius` | > 0 draws rounded corners `╭╮╰╯` on a light solid border, and is refused otherwise | quantize |
| adjacent borders | not joined in v1 (no `├┼┤`); `exact:terminal`'s table draws its own joins | — |
| `overflow` hidden / auto / scroll | clips to cells; scrolls by whole rows and columns; a one-column indicator unless `scrollbar-width: none` | admit |
| `position` relative / absolute / fixed / sticky, `z-index`, top layer (`dialog`, popover) | painted in order over the grid; a dialog traps focus | admit |
| `img` (and an SVG island, rasterized by `exact-svg-raster`) | kitty graphics → iTerm2 inline → sixel → half-block cells (`▀` with fg/bg, at 256 colours or more) → `alt` text; sized in cells; `object-fit` applies inside the cell box | quantize |
| `a href` | OSC 8 hyperlink, and a press follows it (in-app routes navigate) | admit |
| `input`, `textarea`, `select`, `button`, checkbox, radio | host-drawn, D6 | admit |
| `:hover`, `:focus-visible`, `:active`, `:disabled` | D6 | admit |
| `opacity`, `box-shadow`, gradients, `filter`, `backdrop-filter`, `transform`, `clip-path`, `mask` | no cell form | refuse |
| `transition`, keyframes, springs | refused in v1 (as exact1, 0368 §Motion). Spinners and progress are timers and state, which work. Colour transitions at the frame cap are a later candidate | refuse |
| `canvas`, GPU surfaces, `video`, `iframe` | none | refuse |
| `direction: rtl`, bidi text | terminals disagree too widely; out of v1 | refuse |

Admitting double, dashed and dotted borders in the terminal before the native
hosts paint them is a host lagging a row (LLP 1023 D10, "a platform may *lag*
a row's implementation"), not a fork. The web already draws them.

## 5. Testing and oracles

- **The grid is the oracle.** Paint is deterministic, as tiny-skia is for
  Linux pixels. Goldens are `.txt`/`.ans` grids, which are reviewable in a diff.
- **The writer is checked against a parser.** A VT parser (the `vt100` crate,
  in tests only) replays the host's actual bytes into a grid and compares it
  with the host's own. This proves the diffing writer, the synchronized-update
  framing and the escaping (D7) without a terminal.
- **The web as the layout oracle, later.** A terminal plan rendered by the web
  host in a monospace face should snap to the same boxes. That is stage 3 and
  not a gate.
- **A real-terminal matrix, by hand.** Ghostty, kitty, WezTerm, iTerm2,
  Apple's Terminal, Windows Terminal (once Windows lands), and tmux, which
  passes some sequences through and swallows others. It covers capability
  answers, cluster widths and image protocols, it is advisory, and it is
  recorded in the host's spec when there is one.
- **`bun scripts/smoke.mjs terminal`** drives the consumer headless, as the
  other smokes do, including the hostile-string case and the poorest `prefer`.

## 6. Stages

1. **Headless, static.** The schema column for the rows the consumer uses, the
   `terminal` profile in the compiler, `ch`/`lh` in the kernel, the cell
   painter and `.txt` screenshots. The consumer's entry renders headless at
   80×24 and 120×40, driven by `tree`, `layout`, `tap`, `type`, `clock` and
   `screenshot`.
2. **Interactive.** Raw mode, the writer, negotiation, input, focus, scroll,
   fields, resize, suspend, restore, and the dev loop (an edit rebuilds the
   plan and the host restarts it in place, as a native client does on the dev
   URL). `smoke.mjs terminal`.
3. **The rich terminal.** Images, OSC 8 links, OSC 52 copy, styled underlines,
   the web layout oracle, and `exact new --terminal` (LLP 1086).
4. **Later, each on its own consumer:** inline mode, print-once, Windows,
   colour transitions.

## 7. Against the rules it touches

- *"Per-platform sources, never"* (LLP 1023 D10) and *"one route, one file"*
  (DEFERRED §Authoring models). D9 makes the argument and proposes the
  wording. If Charlie doesn't accept it, this RFC has no shape, because
  §1's evidence rules out the one-source alternative.
- *"The component vocabulary never forks"* (LLP 1023 D10). Kept: one schema,
  with one more admission column. `exact:terminal` is composition.
- *"The web is the standard."* Kept: the units are CSS's, the facts are web
  media features, a terminal plan is a legal web page, and the two declared
  deviations (the reverse-video focus indicator and one-cell borders) are named
  in §4 and D6, as LLP 1001 declares the kernel's.
- *"Optional capability is a separate artifact … never a cargo feature on a
  core crate."* Kept: the terminal is another host binary. The kernel gains
  two units every host could use, and no feature.
- *"Every surface multiplies the sweep."* True, and §9's take, owed if this is
  ever built, is what pays for it. The headless host keeps the multiplier cheap: no device, simulator or
  GPU, seconds per run, on any machine.

## 8. Rejected

- **Projecting the GUI entry** (exact1 r1–r13; Charlie's 2026-10-05
  experiment). Tried twice, unusable both times.
- **A terminal-only component package as the primitive vocabulary** (exact1
  r14+). It forks the vocabulary. HTML and CSS names already cover the
  primitives, and the terminal-specific part is composition (D2).
- **Unitless numbers as cells** (exact1). They would mean px everywhere else
  and cells here, which breaks aspect ratios and the web-oracle property (D3).
- **A Rust TUI framework (ratatui) as the renderer.** It brings its own layout
  and widgets, which is a second layout engine beside Taffy, the
  disagreeing-layers class this repo exists to avoid. The host needs a
  terminal writer, not a framework. Its cell buffer and diff are the only parts
  the host would use, and both are small. **Ink** is rejected for the same
  reason (Yoga is the second layout engine) and another: it is React in Node,
  and nothing runs JavaScript above the data seam (DEFERRED §Authoring models).
  Its inline rendering is still the model for D10's inline form.
- **crossterm for input.** It decodes keys and the mouse well, but it is built
  to deliver events, so replies to queries it doesn't know (OSC 11, the kitty
  graphics probe, the cell size, DECRQM) are awkward to get at. Those replies
  are D5's whole job.
- **termwiz** (WezTerm's terminal library) for input and output. It is the
  most complete, with a full parser, capability probing and image cells. It
  is also the heaviest, and it is a framework-shaped layer of its own between
  the painter and the bytes D7 needs to own.
- **A guardian process** (exact1). Deferred to Q5, not adopted.

## 9. What Charlie decides

1. **The consumers. Ruled (Charlie, 2026-10-05):** "in some ways i'd rather do
   todo list than caltrain. an llp reader seems good." Caltrain is not the
   fixture. There are two consumers:
   - **A todo list**, the fixture. It has a list with stable identity, a text
     input, keyboard shortcuts, filters, and storage, which is every D6 path,
     at a size where "does it still work" is answerable in seconds. No todo
     app exists in `apps/` yet. It is terminal-only to start (Q9, ruled):
     `app.json` has only `host.terminal`.
   - **The LLP reader** (`apps/llp`), the terminal-native consumer: long
     Markdown text, wrapping, headings in bold, links as OSC 8, and scroll.
     It's an app that people and agents in this repo would actually open in a
     terminal. It already has a GUI entry, so it is D1's shared-data case.
2. **The take, or a waiver. Not decided** (Charlie, 2026-10-05: "idk, we're
   just exploring here not committing to this"). It is owed only if this is
   built, and it is not the author's to choose.
3. **The D9 wording. Rationale accepted** (Charlie, 2026-10-05: "that
   rationale sounds right"). It enters DEFERRED only with the host.

**Unblocks (the line for DEFERRED):** apps authored for the terminal (full
screen, mouse, images, the same data module and storage as the app's other
surfaces), driven headless by the ten operations.

## 10. Open questions

- **Q1. Same app or separate app?** D1 puts the terminal entry inside the app,
  sharing its id and storage. A Fieldnotes in the terminal should see the same
  notes. The alternative, a separate app over a shared data crate, is cleaner
  for delivery but loses shared storage. Recommendation: same app.
- **Q2. `ch`/`lh` verbosity.** `padding="0 1ch"` everywhere is honest but
  noisy. Could the terminal profile accept bare integers as cells after all, as
  an authoring shorthand that lowers to `ch`/`lh` by axis? That's ergonomic, but
  `padding=1` would then mean different things in two files of one app.
  Recommendation: no, and revisit after the consumer is written.
- **Q3. Unit support elsewhere.** Once the kernel has `ch` and `lh`, are they
  admitted on GUI surfaces too? They're useful (`max-width: 65ch` is the
  readable-measure idiom), but that is a separate admission with its own
  consumer.
- **Q4. Delivery.** Signed bundles per stream (LLP 1030) for a terminal
  binary, or ship the binary and update the plan only? Out of v1, which is
  `exact run terminal` and a built executable.
- **Q5. Restoration after SIGKILL.** Is `reset` an acceptable answer, or does
  the terminal need exact1's guardian? Recommendation: one process until a user
  is bitten.
- **Q6. kitty's text-sizing protocol** (OSC 66) can draw scaled text in
  terminals that support it. Would `font-size` in whole multiples of `1lh`
  become a `quantize` row there? Later, if a consumer wants big numerals.
- **Q7. Borders that join.** Panels side by side with `├┬┤` junctions are what
  a person would draw. Is that `border-collapse` for all boxes in the painter,
  or only in `exact:terminal`'s table? v1: only the table.
- **Q8. Inline mode's settled-row node.** What marks content as finished, and
  how does that read in CSS terms? This needs its own design once a consumer
  wants inline mode.
- **Q9. The todo list's shape. Ruled (Charlie, 2026-10-05: "it can be just
  terminal only to start"):** terminal-only, with only `host.terminal` in
  `app.json`. The LLP reader covers D1's shared-data case. A web entry for the
  todo list can come later, over the same data module.

## 11. What the spike found (2026-10-05)

Branch `terminal-spike` (commits `a28990cdf`, `e93addd52`) built stages 1 and
most of 2 (§6) to see whether D1–D9 hold up in code. They mostly do.

**What runs.** `host/terminal` (`exact-terminal`, about 1,100 lines, outside
`default-members`) boots a terminal entry, lays it out with the kernel at the
fixed cell, paints the kernel tree into a cell grid, and either answers the
agent's verbs headless (`tap`, `type`, `key`, `wheel`, `resize`, `tree`,
`screenshot x.txt|x.ans`, `print`) or runs full screen. `apps/todo/terminal.contract`
is the fixture: filters on `1`/`2`/`3`, a scrolling list in a rounded panel,
`n` to the input, `x` to clear done. Drives and a test in a real
pseudo-terminal show adding, toggling, filtering, Tab/Enter to every control,
the field taking the terminal's cursor, the kitty-protocol Ctrl-C quitting,
and the terminal restored. Updates after the first frame are about 140 bytes
each.

**Findings that changed this document.**

- **D3: fix the cell in the kernel.** `ch`/`lh` are a few lines beside
  `rem`/`em` in the generated `set_dynamic` (`kernel/src/style/cells.rs`).
  Making them depend on the host's real cell size would have meant threading
  host input through bake and runtime for no visible gain.
- **§4: borders need a kernel switch.** The compiler lowers `border` to a
  pixel number, so the kernel never sees `thin`. A border occupies a cell only
  if the kernel knows it is laying out for a terminal. That is a process-wide
  flag only the terminal host sets (`cells::set_terminal`), read in
  `StyleProps::border_widths`. It is the one place the spike touches shared
  kernel behaviour, and it is a declared deviation, as §4 already says.
- **D2: the column needs no generated code.** The compiler reads the
  schema's `terminal` key directly (`contract/cli/src/terminal.rs`) and
  reports every refusal with its span (`terminal-length`, `terminal-refused`,
  `terminal-tag`). The kernel never consults it.
- **Writing for the terminal felt like writing Contract.** The fixture uses
  the ordinary vocabulary in `ch`/`lh`. Its only terminal-specific choice is
  `text-align="start"` on buttons, which CSS's button default centres. Nothing
  new was needed in the vocabulary.

**Confirmed as written:** the painter host with no mirror (D7), our own output
with `vte` for input and `rustix` for the tty (D9), the diff inside
synchronized updates, the kitty keyboard protocol and SGR mouse (D6),
reverse-video focus, and the headless host as the agent's carrier (D8).

**Not built:** the `app.json` `host.terminal` entry (the spike takes a
`.contract` path), the minimum-size screen, capability negotiation and the
web-named facts (D5), colour below truecolor, images, OSC 8, hover,
`scripts/agent.mjs terminal`, the dev loop, and `exact:terminal`. The fixture
keeps its todos in Contract state, with no data module or storage.
