# Review: LLP 1045 Markdown editing and reading, 2026-09-21 (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort=max -s read-only`, one fresh session in the working tree at `eb24ea6` plus the uncommitted slice 1 (`markdown/`, LLP 1045 r1, the proposed NOT-DOING entry). Brief sha256 `0114cfa0bfee3761e5d38d55bbed716a6d7155a606ce02765c9b0fc5b3073c0b`.
- **Method:** a single sanity-check review at Charlie's request ("have astra max sanity check it"), no refine loop. The author (Claude, Fable 5.1) is not a reviewer. Provider: OpenAI; nothing redacted; the brief named the eight questions and asked for HIGH/MEDIUM/LOW with code evidence.
- **Disposition:** by the author, after the verbatim review.

---

## Review (verbatim)

**Overall assessment**

The direction is promising, but I would not continue with slice 2 as written. Keeping Markdown as the stored value and separating cheap reading from editing are sound choices. The RFC understates the editing machinery required, leaves a significant measurement problem unresolved, and overstates the parser’s correctness.

I read the working-tree implementation and ran the existing Markdown test executable: **18 tests passed**. The counterexamples below come from tracing the source; those cases were not exercised by that executable. I did not perform a fresh build or device/browser validation.

**Strengths**

- **D1:** A Markdown string is a good persistence and application interface for Interview: portable, inspectable, and compatible with TypeScript and AI-generated content.
- **D2:** The dependency-free, host-independent crate is a useful boundary. Returning source-relative edits is preferable to having formatting commands replace the entire editor value.
- **D4/D7:** Separating reading from editing, segmenting non-text content, and deferring expensive embeds until activation directly address the owner’s scrolling requirement.
- **D5/D6:** Operating on the live editor selection and fixing controlled-value write-back are the right instincts. Formatting commands can fit the existing command transport without adding an agent operation.

**Concerns**

**1. HIGH — D1 conflates the persisted value with the live editing model.**

The assertion that an attributed model necessarily requires “a new value type on every seam” and a lossy round trip is **false** ([RFC:44](/Users/ccheever/projects/exact2/llp/1045-markdown-editor.rfc.md:44)). A host can retain Markdown as its external value while maintaining an internal attributed document or lossless syntax tree, applying mapped source edits and preserving untouched source.

Keeping source characters in native text storage is feasible, but hiding their glyphs does not specify:

- Whether arrows, word movement, Backspace and Delete skip, reveal, or remove delimiters.
- Whether copying a visible selection produces plain text, balanced Markdown, or the literal source substring. Selecting the label of a link illustrates the difference.
- How autocorrect, dictation and IME replacements interact with syntax inside their replacement ranges.
- Whether typing plus automatic list continuation is one undo step, and whether restyling enters undo history.
- What VoiceOver reads and which positions it can navigate.
- How hidden punctuation affects bidirectional ordering, selection affinity and grapheme boundaries.

The public selection type is only a normalized UTF-16 interval; it does not describe anchor/focus direction or affinity ([lib.rs:46](/Users/ccheever/projects/exact2/markdown/src/lib.rs:46)). Those may remain host-local, but they still need a design.

**Resolution:** Keep the string interface, but distinguish it from the editing representation. Specify these behaviors with examples before choosing glyph hiding as the implementation. “WYSIWYG holds by construction” at [RFC:72](/Users/ccheever/projects/exact2/llp/1045-markdown-editor.rfc.md:72) is also false: a shared parser proves neither editing behavior nor rendering parity.

**2. HIGH — D5 needs an explicit TextKit architecture and physical-iPhone proof.**

Under **TextKit 1**, glyph suppression is feasible: `NSLayoutManagerDelegate` can substitute glyph properties, including null glyphs. That is a rendering mechanism, not a complete selection/input solution. Character indices remain; caret rectangles, hit testing, selection highlights, line endings and accessibility must agree with the hidden presentation. Glyph-generation callbacks also have reentrancy constraints. [Apple’s glyph-generation API](https://developer.apple.com/documentation/uikit/nslayoutmanagerdelegate/layoutmanager(_:shouldgenerateglyphs:properties:characterindexes:font:forglyphrange:)), [glyph properties](https://developer.apple.com/documentation/uikit/nslayoutmanager/glyphproperty).

Under **TextKit 2**, there is no equivalent glyph-generation delegate. Apple documents TextKit 2 as the default for `UITextView` from iOS 16, and accessing `layoutManager` can trigger a one-way compatibility switch that disrupts UI state. Rendering attributes alone do not remove layout advance. [Apple’s TextKit explanation](https://developer.apple.com/videos/play/wwdc2022/10090/).

The existing editor does not explicitly choose an engine ([TextAreaIOS.swift:97](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:97)); its line-height update applies paragraph attributes to the entire storage ([line 42](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:42)). Attaching the proposed styler requires revisiting both.

**Resolution:** Choose TextKit 1 explicitly for the prototype, or describe the TextKit 2 content/layout approach. Prove Japanese/Chinese composition, dictation, autocorrection across formatting boundaries, emoji and combining characters, mixed Arabic/Hebrew text, VoiceOver, selection handles, undo/redo, Dynamic Type, and keyboard-driven scrolling on a phone. Simulator toolbar taps do not establish “excellent on iOS.”

**3. HIGH — The web plan omits much of the editor.**

A dependency-free implementation is possible, but “rebuild spans per paragraph, restore offsets, avoid composition” is insufficient.

Browser edits introduce and rearrange text nodes, `<br>` elements and block wrappers. Their `textContent` is not automatically a lossless newline serialization. Replacing descendants can disturb native undo and composition state. Hidden source nodes require explicit source-to-DOM selection and clipboard handling. Autocorrection and IME changes also do not consistently arrive as cancelable `beforeinput` events. [MDN’s documented limitations](https://developer.mozilla.org/en-US/docs/Web/API/Element/beforeinput_event), [Input Events specification](https://www.w3.org/TR/input-events-2/).

The existing glue assumes `.value` when writing and dispatching changes, and `.select()` for selection commands ([glue.js:431](/Users/ccheever/projects/exact2/host/web/glue.js:431), [line 560](/Users/ccheever/projects/exact2/host/web/glue.js:560), [line 846](/Users/ccheever/projects/exact2/host/web/glue.js:846)). These assumptions stop working with a contenteditable element.

**Resolution:** Before committing, demonstrate the same small editor in Safari, iOS Safari, Chrome and Firefox, covering composition, replacement suggestions, paragraph joins, backward selections, paste/drop, copy/cut, undo/redo and external value updates. Define DOM reconciliation, input fallbacks, clipboard formats, textbox accessibility, placeholder and read-only behavior. Dependency freedom should be an evaluated constraint, not evidence that this approach is simple.

**4. HIGH — D3/D4 do not yet fit the measurement seam, especially when markers reveal.**

The kernel currently measures styled runs with **one paragraph configuration**. `TextMeasureRequest` has no markup discriminator, block sequence or source/display mapping ([text.rs:159](/Users/ccheever/projects/exact2/kernel/src/text.rs:159)). Layout flattens runs and constructs that request directly ([layout.rs:435](/Users/ccheever/projects/exact2/kernel/src/layout.rs:435)). Apple’s bridge likewise forwards runs plus one paragraph configuration ([measure.rs:205](/Users/ccheever/projects/exact2/host/apple/src/measure.rs:205)).

Consequently, adding a measure property does not by itself make the measurer see what the host paints. A segment containing heading metrics, list indentation, code backgrounds and several paragraphs requires a richer measurement/layout contract.

There is an additional editor problem: moving the caret reveals markers and can change wrapping and height **without changing `value`**. Selection is host-local, while cached measurement is reused through the existing layout path ([layout.rs:426](/Users/ccheever/projects/exact2/kernel/src/layout.rs:426)). This needs an invalidation/intrinsic-size mechanism, particularly for `field-sizing: content`.

**Resolution:** Design measurement, painting, cache identity and selection-dependent invalidation together. Define typography against an explicit browser reference, including block spacing and whitespace. A non-CSS `markup` property can be defensible as declared semantic input, but compare it with standard-library expansion into existing nodes and a general attributed-text measurement seam. Keep the Markdown parser out of core kernel dependencies.

**5. MEDIUM — The performance case is plausible, not established.**

Apple already mounts only the paragraph view; inline nodes remain presenter objects and run data, not separate mounted layers ([NodeText.swift:22](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/NodeText.swift:22)). Removing those objects can help, but “fewer nodes” is not equivalent to proportionally less rendering work.

The RFC’s numbers come from research that explicitly marks those exact row-construction timings as **stale on current main**, following changes to text reuse, child lookup and batching ([1044:427](/Users/ccheever/projects/exact2/llp/1044-markdown-scroll-gap.research.md:427)).

A larger segment still shapes its text and maintains link, selection and accessibility information. It also becomes a larger invalidation and virtualization unit. `segments(limit)` never divides a fenced block, and a single large paragraph can exceed the limit ([segment.rs:236](/Users/ccheever/projects/exact2/markdown/src/segment.rs:236)).

On web, the host currently uses browser layout and has no Markdown dependency ([host.rs:172](/Users/ccheever/projects/exact2/host/web/src/host.rs:172), [Cargo.toml:18](/Users/ccheever/projects/exact2/host/web/Cargo.toml:18)). The parser, exports and glue have a real Wasm cost; spans and browser layout remain.

**Resolution:** Measure current nested runs against the prototype: Wasm raw/compressed size, first-pixel work, allocations, scrolling/jumps, edit latency and giant paragraphs/code blocks. Include cross-segment selection, links and semantic accessibility. Do not claim a size or speed win before measuring it.

**6. HIGH — Slice 1 contains concrete editing and preview defects.**

These are source-derived counterexamples:

| Case | Defect and evidence |
|---|---|
| Toggle `CodeBlock` on the empty fenced document `"```\n```"` with the caret at offset 4 | Fence removal produces overlapping ranges `0..4` and `3..7`. `Edit::apply` subsequently slices `4..3` and panics. [edit.rs:329](/Users/ccheever/projects/exact2/markdown/src/edit.rs:329), [line 61](/Users/ccheever/projects/exact2/markdown/src/edit.rs:61). |
| Press Enter after `- item` inside a fenced code block | Prefix detection chooses list continuation before checking code context, inserting another list marker into literal code. [edit.rs:435](/Users/ccheever/projects/exact2/markdown/src/edit.rs:435). |
| Press Enter after `01. a` | Delimiter extraction uses the parsed number’s width, not the authored digits’ width. It produces `21. ` instead of `2. `. Following-item renumbering has the same defect. [edit.rs:473](/Users/ccheever/projects/exact2/markdown/src/edit.rs:473). |
| Apply inline code to text containing a backtick | The command always uses one backtick; it does not choose a delimiter that can represent the selection. Block wrapping similarly hardcodes triple fences. [edit.rs:509](/Users/ccheever/projects/exact2/markdown/src/edit.rs:509), [line 353](/Users/ccheever/projects/exact2/markdown/src/edit.rs:353). |
| Request a four-character excerpt of `[ok](https://e/` followed by a long URL and `)` | Source truncation happens before parsing. Cutting off the closing delimiter turns the link into literal syntax and can return `[ok]…` instead of `ok`. [segment.rs:296](/Users/ccheever/projects/exact2/markdown/src/segment.rs:296). |

**Resolution:** Add these cases to the existing test suite and repair them before host integration. Non-overlapping replacements must be an enforced invariant of every formatting command.

**7. MEDIUM — “CommonMark’s core” and “GFM” overstate the implemented dialect.**

The crate itself explicitly excludes setext headings, indented code and reference links—CommonMark core features ([lib.rs:13](/Users/ccheever/projects/exact2/markdown/src/lib.rs:13)). The RFC does not carry that qualification.

Additional discrepancies include:

- ATX headings do not implement permitted leading indentation or optional closing hashes ([block.rs:123](/Users/ccheever/projects/exact2/markdown/src/block.rs:123)).
- Fences inside block quotes are not recognized as fenced blocks ([block.rs:276](/Users/ccheever/projects/exact2/markdown/src/block.rs:276)).
- Code spans hide delimiters without CommonMark’s newline/edge-space normalization ([inline.rs:139](/Users/ccheever/projects/exact2/markdown/src/inline.rs:139)).
- The tests explicitly require single-tilde strikethrough to remain literal, while the published GFM specification accepts one or two tildes ([tests:134](/Users/ccheever/projects/exact2/markdown/tests/markdown.rs:134)).
- Table segmentation discards alignment information; image segmentation leaves angle brackets in `![x](<a.png>)`’s URL ([segment.rs:213](/Users/ccheever/projects/exact2/markdown/src/segment.rs:213)).

These differ from the [CommonMark specification](https://spec.commonmark.org/0.31.2/) and [GFM specification](https://github.github.com/gfm/).

**Resolution:** Name the actual subset and footnote dialect, with explicit deviations, or run the relevant specification examples and close the gaps. Tests over the repository’s documents establish useful invariants, not conformance.

**8. HIGH — Analysis can be quadratic, and “edited paragraphs” is not the implemented processing model.**

There are several independent problems:

- `flatten` examines all marks for every interval boundary: quadratic in the number of spans in a paragraph ([style.rs:113](/Users/ccheever/projects/exact2/markdown/src/style.rs:113)).
- Each unmatched `[` scans the remaining suffix, making repeated unmatched brackets quadratic ([inline.rs:223](/Users/ccheever/projects/exact2/markdown/src/inline.rs:223)).
- URL punctuation trimming repeatedly recounts parentheses across the remaining URL ([inline.rs:195](/Users/ccheever/projects/exact2/markdown/src/inline.rs:195)).
- Emphasis matching repeatedly searches prior delimiters and shifts a vector ([inline.rs:319](/Users/ccheever/projects/exact2/markdown/src/inline.rs:319)).

`style` rebuilds offsets and analyzes the whole source on every call, including selection-only reveal changes ([style.rs:263](/Users/ccheever/projects/exact2/markdown/src/style.rs:263)). `segments` also performs full inline analysis before cutting ([segment.rs:200](/Users/ccheever/projects/exact2/markdown/src/segment.rs:200)). Whole-string `change` additionally retains an O(document-size) transfer per edit.

**Resolution:** Remove the quadratic scans, retain parsed structure across reveal changes, and define incremental invalidation through changed block context. An opening fence can affect the remainder of a document, so merely restyling the edited paragraph is also incorrect. Establish document-size expectations and benchmark adversarial input alongside ordinary posts.

**9. HIGH — Footnotes are incorrect, and the output types omit document-level information hosts need.**

Numeric labels are treated as display ordinals while named labels use a separate counter. The test actually blesses `[^1]` and `[^note]` both displaying as **1** ([tests:258](/Users/ccheever/projects/exact2/markdown/tests/markdown.rs:258)).

Number allocation also occurs only for hidden references. Revealing the first named reference can renumber later references merely by moving the caret ([style.rs:156](/Users/ccheever/projects/exact2/markdown/src/style.rs:156), [line 225](/Users/ccheever/projects/exact2/markdown/src/style.rs:225)).

Styling independent segments restarts that state. `Replacement::Footnote` contains only the display string, and `Segment` carries no global reference/definition index ([style.rs:63](/Users/ccheever/projects/exact2/markdown/src/style.rs:63), [segment.rs:51](/Users/ccheever/projects/exact2/markdown/src/segment.rs:51)). D8’s document-wide numbering, footer ordering, navigation and backlinks therefore do not follow from these APIs.

More generally, hosts need source/display mappings, semantic links/headings, block grouping and stable segment identity. UTF-16 ranges and style bits alone are insufficient.

**Resolution:** Build a document-wide footnote index independent of selection and segmentation. Carry semantic targets and source ranges through rendering. Design undefined/duplicate references, repeated references, multiline definitions, accessible labels and return navigation before freezing these types.

**10. MEDIUM — D6 fits the transport, but its event and toolbar contract is incomplete.**

Generic command names and arguments already pass through the lowerer and VM ([lower/lib.rs:930](/Users/ccheever/projects/exact2/contract/lower/src/lib.rs:930), [vm.rs:436](/Users/ccheever/projects/exact2/runner/src/vm.rs:436)). `format` fits that mechanism.

However, `select` is new vocabulary: it is absent from the event table and handler allowlist ([format.json:462](/Users/ccheever/projects/exact2/plan/tables/format.json:462), [analyze/lib.rs:218](/Users/ccheever/projects/exact2/contract/analyze/src/lib.rs:218)). That requires coordinated plumbing, but **does not require a ninth agent operation**.

The claim that `"bold heading2"` is all a toolbar needs is false. Mixed selections, link destinations, unavailable commands and link-edit dialogs need more. `retainFocus` cannot preserve selection through a dialog that intentionally focuses another field.

**Resolution:** Specify parameterized commands, atomic undo/change behavior, mixed format states and host-local selection bookmarks. Extend selection observation through existing agent operations where needed. The current iOS `type` implementation selects all before inserting and supports only limited key forms ([AgentIOS.swift:512](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:512)); passing those tests would not prove editing ergonomics or IME correctness.

**11. MEDIUM — D7 needs provider-specific integration, not just generated iframe URLs.**

| Provider | Finding |
|---|---|
| YouTube | The privacy-enhanced URL is reasonable, but URL construction drops playback parameters ([segment.rs:112](/Users/ccheever/projects/exact2/markdown/src/segment.rs:112)). YouTube requires embedded-client identity via HTTP Referer, including app identification for WebViews. Exact’s wrapper currently uses generic `exact.invalid` ([WebArm.swift:69](/Users/ccheever/projects/exact2/host/apple/webarm/WebArm.swift:69)). This needs a deliberate app-identity implementation. [YouTube requirements](https://developers.google.com/youtube/terms/required-minimum-functionality). |
| X | `platform.twitter.com/embed/Tweet.html?id=…` is a hardcoded implementation endpoint ([segment.rs:134](/Users/ccheever/projects/exact2/markdown/src/segment.rs:134)). X’s supported instructions use generated embed code through Publish. Treat the direct iframe as an unsupported assumption, not an established integration. [X instructions](https://help.x.com/en/using-x/how-to-embed-a-post). |
| Instagram | `/p`, `/reel` and `/tv` iframe URLs are synthesized directly ([segment.rs:140](/Users/ccheever/projects/exact2/markdown/src/segment.rs:140)). Public/embed-enabled eligibility and current oEmbed access, review and metadata-use requirements need verification. Meta’s primary documentation was inaccessible during this review; I cannot validate those current requirements. |
| TikTok | The code uses `/embed/v2/{id}` ([segment.rs:161](/Users/ccheever/projects/exact2/markdown/src/segment.rs:161)); the documented iframe player is `/player/v1/{id}`. Short links produce an empty frame URL until resolved. Use a documented integration and define resolution/fallback behavior. [TikTok player documentation](https://developers.tiktok.com/docs/en/embed-player). |

The Apple wrapper currently supplies only iframe source and sandbox attributes ([WebArm.swift:165](/Users/ccheever/projects/exact2/host/apple/webarm/WebArm.swift:165)). Fullscreen permissions, inline playback, sizing and provider messages need consideration.

Cards until activation are good privacy policy, but loading a YouTube poster still makes a third-party request. Privacy-enhanced playback is not a promise of zero data transfer. X explicitly documents data received through embeds. [YouTube privacy-enhanced mode](https://support.google.com/youtube/answer/171780?hl=en), [X embed privacy](https://help.x.com/en/x-for-websites-ads-info-and-privacy).

**Resolution:** Verify each provider on web and iPhone, including attribution/control requirements, unavailable content and offline fallback. Define active-frame eviction and metadata caching. Keep credentials outside shipped TypeScript, isolate provider HTML, and validate navigable URL schemes.

**12. HIGH — The slicing and proposed scope trade do not yet satisfy the owner’s requirements.**

D5 explicitly leaves Linux editing as plain source. That contradicts “WYSIWYG on every Exact platform.” Its justification also contains a factual error: Linux has an end-of-buffer caret ([paint.rs:790](/Users/ccheever/projects/exact2/host/linux/src/paint.rs:790)), although its append/pop editing implementation is far short of selection-based editing ([presenter.rs:1350](/Users/ccheever/projects/exact2/host/linux/src/presenter.rs:1350)).

Essential work is also deferred or unspecified:

- **Write-back:** The unguarded assignments are real on iOS, macOS and web ([iOS:110](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:110), [Mac:51](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:51), [web:431](/Users/ccheever/projects/exact2/host/web/glue.js:431)). Fix this before layering formatting on top. Define deferred external writes and selection transformation, not just “preserve selection.”
- **Paste and media:** Excluding HTML import is reasonable; basic plain-text paste/drop, copy/cut, image insertion/display and link editing still need behavior. Excluding upload does not settle existing image URLs or alt text.
- **Consumer integration:** Interview needs a concrete TypeScript route to segmentation/excerpts, draft recovery and external updates. Slice 6 discovers those constraints too late.
- **Reader migration:** The existing reader resolves relative links and images against the document directory ([data/lib.rs:170](/Users/ccheever/projects/exact2/apps/markdown/data/src/lib.rs:170)). The replacement must retain that behavior before deleting the old parser.

The proposed NOT-DOING entry honestly marks its trade as unconfirmed ([line 95](/Users/ccheever/projects/exact2/rules/NOT-DOING.md:95)). However, LLP 1024 was already deferred behind the reader on September 5 ([line 17](/Users/ccheever/projects/exact2/rules/NOT-DOING.md:17)); deferring it again contributes no new trade. Specific active reader follow-ups can constitute a real take, subject to Charlie’s confirmation.

**Resolution:** State the Linux exception as an owner decision, or include a path to compliant editing. Identify actual work being displaced, and bring a thin Interview integration forward.

**Suggestions**

1. Repair the slice-1 counterexamples, numbering and complexity defects; document the actual Markdown dialect.
2. Move controlled-value correctness and a small iOS/web editing prototype ahead of the permanent kernel property. Exercise hidden boundaries, composition, undo and accessibility.
3. Integrate that prototype into Interview early, using the existing reader where practical.
4. Choose the reader seam after measuring current code and proving measurement parity. Complete provider integrations and reader migration against explicit acceptance cases.

**Open questions for Charlie**

- Does WYSIWYG permit Bear-style syntax revelation and source-only table editing? Is a temporary Linux source editor acceptable?
- Is preserving untouched Markdown byte-for-byte essential, or is semantically equivalent Markdown acceptable after editing? An internal mapped document can support either policy.
- Is “no editor dependency” absolute if the browser prototype cannot meet the input-quality bar within the project’s budget?
- What document sizes and default copy behavior must Interview support, and which active reader follow-ups are actually coming off the doing list?

**Recommended next step**

Revise the RFC and repair slice 1. The next implementation should prove the editing interaction on a physical iPhone and the named browsers before committing to `markup` and the current output types. Those proofs may preserve most of this design, but they address the highest-risk assumptions first.

VERDICT: MATERIAL FINDINGS
---

## Disposition (Claude, 2026-09-21), against LLP 1045 r2 and the working tree

1. **D1 conflates value and editing model — accepted.** r2 D1 keeps the string as the wire value only, allows a host-local editing representation, and fixes the caret/deletion, copy, IME, undo, VoiceOver and bidi behaviours with rules. "Holds by construction" is withdrawn; r2 says what the shared styler proves and what only the runs prove.
2. **TextKit — accepted.** r2 D5 chooses TextKit 1 explicitly (`usingTextLayoutManager: false`), names the glyph-generation mechanism, restyles in place from the edited block down, and slice 3 is judged on a phone with the listed proofs.
3. **Web — accepted.** r2 D5 keeps a source string beside the DOM, names `beforeinput`/`input` reconciliation, clipboard ownership and the `glue.js` branches, and makes "no dependency" an evaluated constraint reported to Charlie (open question in §6).
4. **Measurement seam — accepted.** r2 D3 lands the row with a measurement contract (paragraph sequence in the request, reveal in the cache key, host-local invalidation), keeps the parser out of the kernel crate, and defines typography against the browser.
5. **Performance unproven — accepted.** Stale numbers removed from §2; D4 is stated as a hypothesis and slice 2 is the measurement with the named metrics, including wasm size; the row is kept only on evidence.
6. **Slice-1 defects — fixed, each with a test** (`the_defects_astra_traced_are_fixed`): empty-fence toggle no longer overlaps; Return inside a fence never continues a list or quote; `01.` continues as `2.`; a code span's delimiter is one backtick longer than any run inside the selection; `excerpt` cuts at block boundaries after parsing. Non-overlap of replacements is now an invariant of the fence path; the other commands never produced overlaps.
7. **Dialect overstated — accepted.** r2 D2 names the exact dialect and its deviations; `lib.rs` matches. Single-tilde strikethrough stays literal on purpose (it is the most common accidental match in prose) and is listed as a deviation. Table alignment and `<a.png>` targets: the angle-bracket case is fixed in `figure()`; alignment stays ignored, listed.
8. **Quadratic scans — fixed.** `flatten` is one sweep over sorted boundaries; brackets are matched once per block in a linear pass; URL trimming counts parentheses once; emphasis uses CommonMark's `openers_bottom`. `adversarial_input_stays_linear` runs 20,000-unit cases of each under a 2 s bound (they take ~0.1 s). Whole-source reanalysis per call stays, stated in r2 D2 with the fence argument for restyling to the end; incremental reuse waits for a measured trigger.
9. **Footnotes — fixed and partly owed.** Numbering is by first reference in document order, independent of the selection and of label spelling; `Styled.footnotes` is the index (label, ordinal, references, definition). Per-segment numbering restart is stated as owed in r2 D8 (a text segment must carry its footnote base, slice 6).
10. **D6 contract — accepted.** r2 D6: `select` is declared as new event vocabulary carrying a record (formats, mixed, link target, unavailable commands); commands with arguments; one undo group and one `change` per command; a host selection bookmark across a link sheet.
11. **Providers — accepted, partly fixed.** TikTok now emits the documented `/player/v1/{id}`; YouTube carries a start time and is noted as needing an embedding identity; X and Instagram iframe URLs are marked in r2 D7 as assumptions slice 6 validates or replaces; the poster request is named as a third-party request.
12. **Slicing and trade — accepted.** Linux is now an owner decision (§6); the write-back fix leads slice 2; a thin Interview integration is slice 4; the reader migration keeps relative resolution; LLP 1024 is dropped from the take and the reader's open follow-ups are the trade. r2 §2 corrects the Linux caret claim.

Open for Charlie (r2 §6): Linux source-only editing, copy default, the web dependency question, source-only table editing, the NOT-DOING take.

Proposed next step: revise (done as r2) and stay `Draft`; proceed to slice 2 on Charlie's word, with the measurement deciding the `markup` row.
