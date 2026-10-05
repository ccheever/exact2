# LLP 1045: Markdown editing and reading — one source string, one styler

**Type:** RFC
**Status:** Draft
**Systems:** Text, Kernel (one prop), Contract (one attribute, two commands, one event), Apple host, Web host, Linux host (reading only; editing is v2), the `markdown/` and `markdown/editor/` crates, Interview (first consumer)
**Author:** Charlie Cheever / Claude (Fable 5.1)
**Implementer:** Claude (initial slices); Codex / Astra (editing slice, 2026-09-21)
**Date:** 2026-09-21
**Revised:** 2026-09-22 r4, CodeMirror removed; the editing rules are one crate, an on-demand wasm on the web (r3, 2026-09-21: the source-editing plan)
**Related:** LLP 1001 §6 (text measurement), LLP 1008 §3/§5/§9 (the Apple textarea, keyboard viewport), LLP 1007 (web textarea), LLP 1005 §3 (capability commands), LLP 1020 (`iframe`), LLP 1033 / 1044 (the reader and what its rows cost), LLP 1035.001 D7 (draft lifetime), LLP 1024 (native modules — not used)

## 1. Ask

Charlie, 2026-09-21: a WYSIWYG Markdown editor on every Exact platform, iOS
excellent; output that renders very cheaply so an app can scan through tons of
it, not always in editable form; footnotes, code blocks, and embeds (tweets,
Instagram, TikTok, YouTube); Bear as inspiration. Interview is the first
consumer and may change to fit; it lives here as a standard-library candidate.

Charlie approved the next implementation slice on 2026-09-21: dependable
source editing first, TextKit 2 on Apple, CodeMirror 6 on the web, syntax
revealed across the active paragraph, and a thin Interview compose screen.
The remaining media, footnote, highlighting and reader migration slices
remain proposed work; this approval does not claim their completion.

### Product direction refined — 2026-09-21

[confirmed] Charlie requested true WYSIWYG as the default, an advanced mode
for editing Markdown directly, and direct Markdown access through the API.
This supersedes active-paragraph syntax reveal as the intended default UX;
it does not claim that the replacement editor has been implemented.

The normal editor keeps formatting rendered while the caret or selection is
inside it. Formatting controls, typing, deletion, selection, paste, and undo
operate as rich-text editing. Hiding markers on a source editor alone does not
satisfy this behavior. An explicit **Edit Markdown** mode exposes the complete
source; switching modes without editing must preserve it, including constructs
the rich editor does not support. Unsupported constructs need a lossless
representation, rather than being silently removed or normalized.

The stored value and public read/write API remain Markdown strings. A host
may use a rich document internally, but that representation does not become
the application or API format. An API client can read the Markdown, change it,
and write it back without driving the visual editor. Formatting-command API
expansion is a separate choice, not implied by direct source access.

[owed] The TextKit implementation is still the earlier hybrid source editor;
the web now runs the WYSIWYG rules (below). Apple needs them, and both need
phone and Safari proof of formatting boundaries, deletion across styles,
lists, links, paste, composition, undo/redo, and lossless source-mode round
trips.
The decisions below describe the existing implementation where they still
refer to automatic syntax reveal. This document remains Draft.

### Web editor and shared rules — 2026-09-22

[confirmed] Charlie, 2026-09-22: remove CodeMirror; the web editor is the
browser's own `contentEditable`, as the MD Lab prototype was (“let's remove
codemirror”). The editing rules move out of each host into one crate,
`exact-markdown-editor` (`markdown/editor/`), kept apart from the reader so
that “apps that don't use a markdown editor [don't] have to pay the cost of
that code weight… you are paying the minimum cost.”

[implemented, web] The crate owns what the platform must not decide: which
syntax is hidden and where a caret may sit beside it, where typed text lands
at a style boundary, what Backspace removes, pending formats at a caret, list
Return, commands, and undo. It draws nothing; a host draws its lines, one per
source line, with hidden syntax kept out of layout (`display: none` on the
web), so the drawn text is the source and plain typing, composition and
spelling run natively and are read back. The web ships it as its own wasm
beside `markup-editor.js`, fetched only when a Markdown textarea mounts: 153
KiB (68 KiB gzip) against the CodeMirror bundle's 271 KiB of JavaScript (87
KiB gzip), and the main wasm drops its editing exports for every app (36 KiB
raw, 15 KiB gzip, measured on `markdown-stress`). The reader's styler stays in
the main wasm, so an app that both reads and edits downloads the parser twice.
Driven in headless Chrome: 28 scenarios (pending formats, boundaries, deletion,
lists, links, composition, paste, undo, the link sheet's bookmark); Safari,
iOS Safari and Android remain the go/no-go. [owed] Apple adopts the crate
through its C ABI, replacing the hybrid editor.

## 2. Starting point before these slices

- Editable text is one controlled `value: String` on `TextInput`; the host owns
  the native field (`UITextView`, `NSTextView`, `<textarea>`), sends the whole
  string per change, and writes `value` back only when it differs. No selection,
  no attributed value, no composition guard on that write. Linux's editor
  appends to or pops the prop and paints a caret at the end of the buffer; it
  has no selection and no caret movement.
- Styled reading text is nested `text` runs. Each run is a runner node and an
  unmounted presenter object; LLP 1044 found paragraphs of many runs to be the
  long fill units. Its per-view timings are marked stale on current main
  (1044 §8.1), so this document claims no number until slice 2 measures one.
- `apps/markdown/parse` is a hand-written CommonMark subset: no footnotes,
  strikethrough, task lists or nesting. Its `Blocks`/`Runs` components are
  copied between three apps because `use` cannot leave an app directory.
- `NativeView` (LLP 1024) is a reserved node type with no implementation.

## 3. Decisions

### D1 — The value is Markdown source, and nothing else crosses the wire

Bear's model: the buffer *is* the Markdown; markers are hidden, not absent.
The editor's `value` stays a string, `change` stays the whole string, the agent's
`type` stays a paste. No rich value type, no document tree in plan state, no
serializer whose normalization could rewrite what a person typed. Storage,
search, diffing, AI-written posts and copy/paste are all the same text.

Refused as the *wire* value: an attributed-runs model serialized to Markdown on
change, which puts a new value type on every seam and a serializer between the
person and their text. A host may keep whatever editing representation it
needs behind the string — an attributed storage with the source's characters,
or a mapped tree — as long as untouched source survives byte for byte. The
editing behaviours the string does not decide are fixed here, not left to
each host:

- **Caret and deletion.** The active paragraph's syntax is visible, dimmed
  text; a selection reveals the paragraphs it touches. This is the initial
  editing policy approved on 2026-09-21. Hidden markers outside those
  paragraphs are skipped by arrow, word and line movement as if absent;
  entering their paragraph reveals them, after which movement and deletion are
  character by character through the syntax. Backspace at the start of a
  hidden construct's text reveals rather than deletes. Deleting one of a pair
  leaves the other visible as literal text — never silently removes it.
- **Copy.** A selection copies its literal source substring (Charlie,
  2026-09-21). A second copy command copies `plain()` of it — a modifier
  chord on macOS and the web, an edit-menu item on iOS — for pasting where
  syntax would read as noise. The chord is ⌘⇧C on macOS and Mod-Shift-C on the web; iOS
  exposes “Copy Plain Text” in its edit menu.
- **Autocorrect, dictation, IME.** Replacements land in the source; the styler
  reruns after. A marked (composing) range is never restyled and the `value`
  write-back never happens while it exists.
- **Undo.** The system's undo; a `format` command's replacements are one
  group, and Return's list continuation is one group with the newline. On
  Apple, a genuinely different external value resets only that editor's undo
  history, because its old native ranges cannot be safely reused; an ordinary
  controlled echo retains history. The web editor resets on a genuinely
  different external value too.
- **Accessibility.** VoiceOver reads the styled text with hidden markers
  omitted and traits (heading, link, code) set; revealed markers read as text.
- **Bidirectional text.** Markers are ASCII punctuation; hiding them removes
  neutral characters, which cannot change the base direction of a paragraph.
  Mixed-direction selection across a hidden range is proven on the phone, not
  assumed.

### D2 — One crate, `markdown/` (`exact-markdown`), depending on nothing

Pure functions over `&str`, UTF-16 offsets out (what CoreText, TextKit and the
DOM count in):

- `style(source, reveal) -> Styled` — spans (range + inline style bits + link),
  paragraph styles (heading level, list depth and marker, quote depth, code
  block, footnote definition), **hidden ranges** (the markers), and replaced
  ranges (a footnote reference shown as a superscript ordinal, a task box, a
  rule). `reveal` is the selection: every paragraph it touches keeps its
  markers visible. `reveal = None` is the reader: everything hidden.
- `edit(source, selection, command) -> Edit` — `bold`, `italic`, `code`,
  `strike`, `link`, `heading(n)`, `bullet`, `ordered`, `task`, `quote`,
  `codeblock`, `footnote`, `indent`, `outdent`, `toggleTask`, and the input
  rule `newline` (continue or end a list/quote, renumber). Returns replacement
  ranges and the new selection, so a host applies it as ordinary undoable edits.
- `segments(source, limit)` — the source cut at block boundaries into text
  segments and the blocks text cannot be: image, embed, table.
- `plain(source)`, `excerpt(source, chars)` — for feeds and search.

**Dialect,** named exactly: ATX headings (no closing hashes, no indent),
paragraphs, `*` `_` emphasis with CommonMark's flanking and rule of three,
code spans (no edge-space normalization), `~~` strikethrough (two tildes; one
stays literal), inline links, autolinks and bare `http(s)` URLs, backslash
escapes, fenced code (not inside quotes), `- * +` and `1.` `1)` lists with
`[ ]` tasks, block quotes by depth, thematic breaks, GFM pipe tables (alignment
row required and ignored), footnote references and single-paragraph
definitions, and figures (D9). **Not** setext headings, indented code,
reference links, HTML (inert text), nested block structure beyond depth
numbers, or multi-paragraph footnotes. This is the subset Interview needs; the
CommonMark spec examples for the constructs above are the conformance bar
when a host integrates, and each deviation stays listed here.

**An embed is a paragraph that is only a URL of a known provider** (YouTube,
X/Twitter, Instagram, TikTok), so the source stays portable Markdown.

Reading and editing call the same `style`, which makes the *styling* agree;
editing behaviour (D1) and rendering parity are proven by the slices' runs,
not by construction. `style` is whole-source; an opening
fence changes everything below it, so a host restyles from the edited block
to the end of the source, not the edited paragraph alone. Incremental reuse
is a later optimization with a measured trigger. Delimiter pairing keeps
processed entries in an active prefix of its input buffer; retiring a match
leaves unread delimiters in place, preserving partial runs and opener-search
boundaries without repeatedly compacting the tail. Autolink searches reuse
the next terminator and last `@` within each inline range; scheme validation
stops at the first invalid prefix byte. Rejected starts therefore do not
repeatedly search the same suffix. Code spans reuse closers found by the
bracket pass; a failed search with later runs builds a temporary length index
in the same lookup buffer. Ordinary matched spans and a final unclosed run
need no such index. The footnote-numbering prepass parses only blocks
whose content contains a `[^` opener; other blocks need just the styling pass.

### D3 — One kernel row: `markup`

`markup` (`none` | `markdown`), a measure prop on `Text` and `TextInput`.
Contract: `text post.body markup="markdown"` and `textarea … markup="markdown"`.
The host, which already links Rust, styles the node's own string. Not CSS —
there is no CSS for this; declared in LLP 1001 with this reason when the row
lands. The parser stays out of `exact-kernel`: the kernel carries the prop,
the hosts link `exact-markdown`.

What the row does not do on its own: the measurement seam today carries one
paragraph configuration and a flat run list (`kernel/src/text.rs`
`TextMeasureRequest`; `host/apple/src/measure.rs`). A marked-up segment is a
*sequence* of paragraphs with their own metrics, indents and spacing. So the
row lands with a measurement contract: the request for a `markup` node
carries the styler's paragraphs and spans, the host measures that sequence
with the same code that paints it, and the cache key includes the reveal —
because moving the caret reveals markers and changes height without changing
`value`. A host-local reveal change invalidates that node's measurement
(`field-sizing: content` sees it as an intrinsic-size change). Typography is
defined against the browser: a `markup` node on the web is spans in one
element, and the native block spacing, list indents and code backgrounds are
held to that rendering by the parity fixtures the reader already has.

### D4 — The reader is one node per segment

`text markup="markdown"` paints a whole segment — headings, lists, quotes, code
blocks, footnotes — as one attributed paragraph sequence in one view: CoreText
on Apple, one element of spans on the web, one cosmic-text buffer on Linux. A
typical post is one segment, so one node, against today's 1 + runs per block.
Images, videos, embeds and tables are their own segments; a huge document is
segmented by `limit` and windowed by the existing measured list. A feed card
is `excerpt`, one plain node.

Reader navigation accepts parsed `http`, `https`, `mailto`, and `tel` targets.
Other schemes render as inert labels; their canonical source and editable
destination stay intact. The web resolves relative links against the document
base before checking the protocol. Native readers have no document base, so
relative destinations remain inert there.

**Landed 2026-09-21 (slice 2, first form).** The kernel carries `markup` and
`Paragraph.markup`; `exact-markdown::pieces` flattens a source into display
runs (headings as bigger bold runs, bullets, boxes, rules, quote bars and
footnote marks as glyph runs, a short line between blocks) that the existing
run painters draw, through `exact_markup_pieces` on Apple and a JSON prop on
the web — one function for measure and paint. Flattening advances through
sorted style, hidden and replacement ranges, collecting cut boundaries only
from ranges intersecting each block. Block decoration that runs
cannot express (code backgrounds, real quote bars) is owed to a later form
that carries paragraph attributes. **Hanging indents landed 2026-10-04**
(the notes diary) as the one paragraph attribute pieces carry: a list item's
pieces hold its head indent, 40 px per level as the UA sheet's
`padding-inline-start` on `<ul>`/`<ol>`, and its marker (disc, circle,
square by depth; the source's number; a task box) is a piece that hangs
before it, its end at the indent, as an outside marker sits. The web makes
the item a block padded by the indent with the marker in a 40 px box pulled
back by a negative `text-indent`; Apple and Linux start each line at the
indent, the first less the marker's shaped width, in the code that both
measures and paints (`LineInsets`, `shaping::line_insets`). Chrome's own
`<ul>` is the oracle (TextParityMacTests). Linux reads through `markup` from
the same date, and follows a reader's links as Apple does: a path naming a
route navigates, `http`/`https`/`mailto`/`tel` would leave the app and,
with no browser on the host, are logged. Measured in
`apps/markdown-stress` ("One markup node" against "Render ALL blocks", same
generated document, three rounds, medians, agent acknowledgement times, an
M4 Pro at 60 Hz): iOS simulator 256 KiB — 8,805 nodes → 61, mode switch to
settle 692 → 133 ms, a 4,000 pt wheel to settle 48 → 27 ms; 1 MiB — 35,105 →
61, 3,015 → 1,020 ms, 151 → 34 ms. Web (Chrome) 256 KiB — 238 → 133 ms,
64 → 40 ms; 1 MiB — 830 → 735 ms, 145 → 42 ms; the wasm grew 26 KiB raw,
5 KiB gzipped. The row stays. A single node is not windowed, so a 1 MiB
document still lays out whole; segments over the measured list are the
design for that (below).

This was the performance *hypothesis* before that measurement: Apple already mounts only
the paragraph view and keeps runs as data, and a bigger segment is a bigger
invalidation and selection unit; a single paragraph or fenced block larger
than `limit` stays one segment. Slice 2 measures the prototype against the
current reader on the same documents — fill-unit time, views created, wasm
size raw and compressed (the web host has no Markdown dependency today),
first-pixel work, jumps, selection across segments, links, accessibility —
and the row is kept only if the numbers say so.

### D5 — The editor is the platform's text view with the styler attached

- **iOS / macOS:** the existing `UITextView` / `NSTextView`, storage = source,
  on **TextKit 2** (Charlie, 2026-09-21). The source-visible editing slice
  replaces the iOS prototype's near-zero-font hiding: every character keeps
  its normal advance, with inactive syntax dimmed. Shared styling lives in
  `host/apple/Sources/ExactKit/MarkupEditor.swift`; the platform adapters apply
  edits through native text insertion and undo. Neither adapter touches
  `layoutManager`, which would switch back to TextKit 1.
  Typing attributes reset to the base after restyling. The host restyles
  attributes in place after text or selection changes and skips marked text
  throughout composition. `format`, `select`, list Return, selection bookmarks
  across a link field, and alternate plain copy are implemented on both hosts.
  Native markers remain visible until caret movement, selection gestures and
  accessibility have been judged on a phone. The current proof uses a fixed
  editor height; intrinsic `field-sizing: content` parity remains owed.
- **Web:** `contentEditable`, no library (Charlie, 2026-09-22). Each source
  line is a block; hidden syntax is a `display: none` span, out of layout,
  caret movement and the accessibility tree. `beforeinput` goes to
  `exact-markdown-editor`, which prevents and applies structural edits
  (Return, deletion, insertion at a style boundary, paste, pending formats)
  and lets plain typing and composition through; the host reads the DOM back
  and redraws only lines that changed, keeping lines the platform already
  typed correctly so autocorrect state survives. The editor owns undo.
  `markup-editor.js` and `markup-editor.wasm` load after first pixel when a
  Markdown textarea mounts; reading loads neither. Removing `markup` swaps in
  a plain textarea with the same source: the explicit source mode.
- **Linux:** v2 (Charlie, 2026-09-21: "focus on web ios macos for now and
  worry about linux in v2"). It reads through `markup` when the row lands,
  since the painter already links Rust; editing stays the existing plain
  end-of-buffer editor until a Linux lane takes it up.
- The `value` write-back is fixed first, in slice 2, on every host, before any
  styling is layered on it: a write arriving during composition is deferred
  until the composition ends; a write that differs from the buffer applies as
  a minimal replacement (common prefix and suffix kept) with the selection
  transformed through it, so an app's own edit to `value` does not throw the
  caret to the end.

**Native validation status, 2026-09-22:** UIKit must retain its specialized
per-editor undo manager; replacing it with a plain `UndoManager` can leave
source unchanged on Undo. Source undo/redo across natural run-loop turns and
editor-history isolation are verified in the simulator with the native manager.
The one-command-one-undo requirement remains incomplete when another source
edit and a format share an automatic native event group: Undo can restore the
source preceding both edits. A manual automatic-group repair was reverted
after a scheduled run-loop close raised an exception on iOS and macOS. It is
not part of the editing baseline. Phone interaction and intrinsic editor sizing
remain separate, unproven work.

### D6 — Commands and toolbar state

`format(id, command, argument?)` joins `focus(id)` and `selectText(id)` as a capability
command (the lowerer and VM already pass generic command names and arguments);
the host runs `edit` against its live buffer and selection and applies the
replacements as one undo group and one `change`. Commands with arguments use
the third string: `format(id, "link", url)`, `format(id, "heading", "2")`,
or `format(id, "figure", src)`.
A `select` event is new event vocabulary (the event table and handler list
gain one entry) and carries a `MarkdownSelection` record: `formats` (space-
separated command names, headings spelled `heading1` through `heading6`), `mixed`
when a selection spans differing formats, `link` (the target under the caret,
or empty), and `unavailable` (space-separated commands; inside a fence, most are).
That is what a toolbar, a link sheet and an agent assertion need; no range
crosses the wire. The toolbar is authored Contract (`retainFocus`), riding
the existing keyboard viewport on iOS. A link sheet that focuses its own
field cannot keep the editor's selection by `retainFocus`; the host keeps a
selection bookmark for the editor across `focus` and restores it when the
sheet's `format(link(url))` arrives. ⌘B/⌘I/⌘K and the iOS edit menu are
host-native. No ninth agent operation: the agent drives editing with `tap` on
the toolbar and `type` with `key`, and reads `select`'s record through `state`.
Passing those drives proves the wiring; the iOS ergonomics bar is a phone.

### D7 — Embeds are cards until asked

The runner does no I/O, so a segment carries provider, id and URL only. The
reader shows a card (YouTube's poster is derivable from the id — and loading
it is a request to Google, so an app that promises no third-party requests
before activation renders the card without it; the rest show provider and
URL, or title/thumbnail the app supplies from its own oEmbed fetch).
Activation mounts an `iframe` (LLP 1020) on web and Apple; Linux opens
nothing. Scrolling past a hundred embeds costs a hundred cards.

Provider facts, to be verified on web and iPhone in slice 5 and not before
claimed: YouTube's `youtube-nocookie.com/embed/{id}` needs an embedding
identity (HTTP Referer; the Apple wrapper's `exact.invalid` origin is not
one) and its terms' required controls; TikTok's documented player is
`/player/v1/{id}`, and a `vm.tiktok.com` short link is an unresolved card
until the app resolves it; X and Instagram publish embed *code* (a script and
a blockquote), and the direct iframe URLs the crate emits for them are
assumptions that the slice validates or replaces with a wrapper document the
host owns. Fullscreen, inline playback and sizing messages from the frame are
the wrapper's; the active frame is evicted when its card leaves the window.

### D8 — Footnotes and code

A reference is a replaced range: a superscript ordinal in both modes, the
`[^label]` revealed under the caret. Numbers are by first reference in
document order whatever the label says (`[^7]` referenced first is 1, as GFM
draws it), never by what the selection reveals; a definition with no reference
numbers after every referenced one; a reference with no definition draws `?`.
`Styled.footnotes` is the index: label, ordinal, every reference range, the
definition paragraph. Definitions style as a footnote paragraph where they
are written; the reader orders them at the end of the last text segment, and
because numbering is document-wide and segments are styled one at a time, a
text segment must carry its footnote base — owed in slice 5 (today `segments`
restarts numbering per segment). Code blocks are monospace paragraphs with a
block background and `pre-wrap`, **highlighted** (Charlie, 2026-09-21: "i
want to do syntax highlighting in code blocks"): the styler tokenizes a
fenced block by its info string and emits spans with a `token` class —
keyword, string, comment, number, type, punctuation — that a host maps to
colours from the theme; no flags bit is spent. The tokenizer lives in the
crate, dependency-free, for a fixed set of languages (Rust, Swift,
TypeScript/JavaScript, Python, JSON, shell, HTML, CSS, Markdown); an unknown
language is plain monospace. Highlighting is a reading feature that the
editor shows too, since both call `style`.

### D9 — A figure is a lone image, video or embed, and its caption is the link text

Charlie, 2026-09-21: embeds, images and videos need captions. The source
already has a slot for each, so no syntax is added:

- `![Caption](photo.jpg)` alone in a paragraph is a figure; the bracket text is
  drawn beneath it and is its accessible name (Pandoc's implicit figure, the
  convention most Markdown tools already render this way). An image inside a
  paragraph is still an inline image with alt text only.
- `![Caption](clip.mp4)` — an image whose target ends in a video extension
  (`mp4`, `mov`, `m4v`, `webm`) is a video figure, played by LLP 1042's player.
- `[Caption](https://youtu.be/…)` alone in a paragraph is a captioned embed;
  the bare URL form of D7 is an uncaptioned one.

Every form degrades in any other renderer to a link, an image with alt text,
or a URL. In the editor a figure is a replaced range drawn as the figure with
its caption editable in place; the caret in the caption reveals the syntax.

### D10 — Media upload: the host picks, the app stores, the editor inserts a URL

An `upload` capability command — `upload(id, "image" | "video")` — asks the
host for media: PHPicker on iOS, an open panel on macOS, `<input type=file>` on
the web, refused on Linux. The picked file arrives through the app's data seam
as an app-scoped file (LLP 1030.002) the app's logic uploads or keeps; the app
then issues `format(id, figure(url))`, which inserts `![](url)` as its own
paragraph with the caret in the caption. The editor never sees bytes; the
runner does no I/O; where a file goes is the app's decision. Progress is the
app's state (a placeholder URL the app later replaces in the source).

This takes `fileinput` off `rules/DEFERRED.md` in this form only — a
picker limited to images and video, no camera, no generic file input.

## 4. Slices (each verified by running)

1. `markdown/`: `style`, `edit`, `segments`, `plain`, `excerpt`, with tests
   over the dialect's cases, adversarial input, and this repository's own
   documents. *2026-09-21, r2 after review.*
2. **Write-back and a measured reader prototype.** The `value` write-back fix
   on every host (D5). `markup` on `Text` behind the measurement contract of
   D3, on Apple and web first, over the current reader's documents, measured
   against nested runs (D4). The row is kept or dropped on that evidence.
3. **iOS editing prototype on a phone.** TextKit 2 styling, paragraph reveal,
   D1's behaviours — Japanese and Chinese composition, dictation, autocorrect
   across a boundary, emoji and combining characters, Arabic/Hebrew, VoiceOver,
   selection handles, undo/redo, Dynamic Type, caret reveal under the keyboard
   — `format`, `select`, the toolbar. Driven on the simulator; judged on a
   phone.
4. **Thin Interview integration**, as soon as 3 renders: the compose screen on
   the prototype, post bodies through `markup`, feed cards through `excerpt`,
   drafts through the existing `value` path. This is where the TypeScript
   route to `segments`/`excerpt` (a data-seam call into the crate, LLP
   1027.001's shape) and external updates are found, not in the last slice.
5. macOS (shared Swift) and the web editor with its browser
   evaluation (D5); the web prototype runs alongside the native editing slice.
6. Embeds verified per provider (D7), figures with captions, footnote base per
   segment, tables as a segment, code-block highlighting (D8), plain-text
   paste and drop, link editing.
7. Media upload: the picker on iOS, macOS and web, and figure insertion.
8. `apps/markdown` moves onto segments, keeping its relative link and image
   resolution against the document's directory; `apps/markdown/parse` is
   deleted only then.

## 5. Not in this design

Collaborative editing, a rich paste importer (HTML → Markdown), a camera or
generic file input, image editing, highlighting beyond the fixed language
set, table editing beyond
source (v1 types pipes; v2 owes a grid editor or a cell-aware `format`), a Contract component
library (`use` across apps stays as QUEUE has it), LLP 1024, and a separate rich
document model. The web editor has no dependency (Charlie, 2026-09-22).

## 6. Open, for Charlie

- Decided 2026-09-21: the `rules/DEFERRED.md` take is the reader's other
  open follow-ups (heading anchors, file watching, the compact folder layout);
  code-block highlighting is in v1 (D8). LLP 1024 was already deferred on
  2026-09-05 and is not a new trade.
- Decided 2026-09-21: copy defaults to source with a plain-text alternative
  (D1); Charlie approved CodeMirror 6 for the web editing slice, then removed
  it on 2026-09-22;
  tables are source-only in v1 and v2
  owes a real answer (§5).
- `llp/current/` is at 15; this document is not linked there until one leaves.
