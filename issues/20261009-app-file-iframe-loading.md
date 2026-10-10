# Show a PDF: an `iframe` of an `app:/` file, and a PDF element fitted to its width (rest of #115)

**Status:** Open
**Systems:** web hosts, host/apple web arm, storage
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/273

## Current scope

Load permitted app:/ documents through typed paths, honoring sandbox/origin policy, URL cleanup and errors. Prove iOS PDF pixels as well as tree state. Separate fitted PDF/object/PDFKit surface stays deferred.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #115. #205 fixed one part: a bundled (`assets/`) PDF in an `iframe` is now shown by WebKit's PDF view on macOS and iOS, and the web serves `.pdf` as `application/pdf`. Two parts remain, both listed as open points of #205:

- **An `iframe` of an `app:/` file.** A PDF (or any document) the app wrote under `app:/data`, `app:/cache` or `app:/tmp` cannot be shown. `image`, `video` and `audio` already take `app:/` sources; `iframe` does not.
- **A PDF element.** An app that previews attachments shows a PDF's pages fitted to the panel width, with no toolbar, scrolling, selection and find, and needs `load` and `error` to show its own loading and failure states. On the web this is `<iframe src="x.pdf#toolbar=0&view=FitH">` (Chrome's viewer) or `<object type="application/pdf" data>`. Contract has no `object` or `embed`, and WebKit's PDF view ignores the `#toolbar=0&view=FitH` fragment and draws its own surround, so a fitted page needs either an element or a defined mapping.

Without them an app writes a native module around `PDFView`, which the agent cannot read or drive.

### Current and expected behavior

On main `0365ad1a4` (`host/web-js/rt.js`, `WebModule.swift` and the web arm are unchanged on `e200397ec`), with a data module that writes a one-page PDF to `app:/data/media/doc.pdf`:

- **Web:** `iframe src="app:/data/media/doc.pdf"` becomes `about:blank` (`tree`: `WebView#7 [pdf-app] url="about:blank"`); `rt.js:663,673` passes an iframe `src` only for `http:`, `https:`, `mailto:` and `tel:`. The bundled `assets/doc.pdf` frame beside it shows "Hello PDF" in Chrome's viewer.
- **macOS:** the same `iframe` keeps `url="app:/data/media/doc.pdf"` and shows nothing (no guest); `WebModule.swift:217` reads only a scheme-less `src` from the bundle and hands any other to the web view, which has no `app:` handler. The bundled frame shows the page (guest `div#annotationContainer`).
- On both hosts the empty `app:/` frame still fires `load` (on the web, `about:blank`'s), so an app cannot tell it failed.
- `contract vocab object` / `embed`: "is not a tag or an attribute".

Expected: an `iframe` of an `app:/` file shows it as the same file served by its type does (the web: an object URL of the file; Apple: the file loaded with its type, as #205 does for `assets/`), and a missing file fires `error`. And a way to show a PDF fitted to the element's width without a toolbar, with `load` and `error`.

Proposals, not requirements: A, `iframe` takes `app:/` sources as `image` does; B, `object type="application/pdf" data=…` (HTML's element) backed by PDFKit's `PDFView` on macOS and iOS (`autoScales`, single page continuous), Chrome's viewer on the web, refused with a journal line on Linux; or C, keep `iframe` and define what the PDF open parameters (`#toolbar=0&view=FitH`) mean on Apple.

### Reproduction and evidence

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| `app:/` PDF in an iframe, web | `bun scripts/exact.mjs new <dir>`; `grants = 'fs.read app:/data/media\nfs.write app:/data/media'`; a source `writeFiles` that `storage.fs.mkdir('app:/data/media')` and `writeFile('app:/data/media/doc.pdf', <583-byte one-page PDF>)`; a `mutation written` sent by a button; when written, `iframe src="app:/data/media/doc.pdf" width=320 height=200 load=appLoaded testId="pdf-app"`; beside it `iframe src="assets/doc.pdf" … testId="pdf-asset"`. `bun exact.mjs agent web --storage x29 "clock data" "tap write" "clock data" "clock +2500 real" tree screenshot` | Chrome 154 (headless, agent) | main `0365ad1a4` | `pdf-app url="about:blank"`, blank; `load` ran once; `pdf-asset` shows "Hello PDF" | `pdf-app` shows "Hello PDF" | `tree`, screenshot |
| The same, macOS | `bun exact.mjs mac`, then the same drive with `agent macos` | macOS 26.6.2 | main `0365ad1a4` | `pdf-app url="app:/data/media/doc.pdf"`, no guest, blank; `load view 7 (appLoaded)` in the journal; `pdf-asset` shows the page | `pdf-app` shows the page; a missing file fires `error` | `tree`, `logs`, screenshot |
| PDF element | `bun exact.mjs contract vocab object` / `embed` | compiler | main `0365ad1a4` | not a tag | an element (or mapping) that fits a PDF to the width with no toolbar | vocab output |

### Acceptance criteria

- On macOS, iOS and the web an `iframe` of an `app:/data` PDF renders its first page, `tree` shows it loaded, and an `app:/` source naming no file fires `error`.
- A PDF can be shown fitted to the element's width with no toolbar, with `load` and `error`, on macOS and the web (Chrome as the oracle); an AppKit test checks selection and find in it.
- Linux refuses the PDF form with a journal line naming it.

### Constraints and related work

- Related: #115 (closed by #205: bundled PDF iframe; `.pdf` served as `application/pdf`); #205's open points list both remaining parts.
- `rules/DEFERRED.md` §Components (restated 2026-09-27): a built-in tag is an HTML element with HTML's meaning; `object` would be one, and the tag needs that reading confirmed.
- LLP 1020 §10 (local documents served at `http://exact.localhost`) is where an `app:/` file would be served on Apple.
- Not tested: iOS, Linux, a non-PDF `app:/` document.

## Discussion at transfer

### daehyeon-mun — 2026-10-08T04:17:52Z

## Decision needed

**What blocks it (main `0365ad1a4`).**
- Element: `rules/DEFERRED.md:427-431` (§Components): "roughly 15 built-in tags, not 40", restated 2026-09-27: "A built-in tag is an HTML element with HTML's meaning … Nothing that isn't HTML is added by it." `object` is HTML's, but no ruling has admitted it, and #205 left "a PDF element with a fit-to-width mode and `load`/`error`" open.
- `iframe` of `app:/`: no rule refuses it. The web host's navigation policy (`host/web-js/rt.js:663,673`) writes `about:blank` for any `src` that is not `http:`, `https:`, `mailto:` or `tel:`; Apple reads only scheme-less sources from the bundle (`host/apple/Sources/ExactKit/WebModule.swift:217`).

**Options.**
- **A. `iframe` takes `app:/` (no ruling needed) plus `object type="application/pdf" data=…`.** `object` is backed by PDFKit's `PDFView` on macOS and iOS (fit to width, continuous, no toolbar) and by Chrome's viewer on the web, with `load` and `error`; Linux refuses it.
- **B. `iframe` takes `app:/` and the PDF open parameters.** No new tag: on Apple a PDF frame honours `#toolbar=0&view=FitH` (and `page=`) by configuring WebKit's PDF view or swapping in `PDFView`; the web passes them to Chrome.
- **C. `iframe` takes `app:/` only.** A fitted, chrome-less PDF stays an app native module.

**Recommendation.** A. The `app:/` source is the same rule `image`, `video` and `audio` already follow, and `object` is HTML's own element for embedded documents, so it fits §Components' restated reading without inventing a tag; PDFKit gives selection and find that the agent and an AppKit test can check. B keeps the tag count but makes Apple interpret a Chrome-specific fragment.

**Cost.** `app:/` iframes: the web resolves the file to an object URL as `image` does, and Apple loads it through the web arm's typed local-file path (#205's `serveFile`) from the data root; a missing file fires `error`. `object`: one tag lowering to a document node, a PDFKit presenter on macOS/iOS, `<object>` on the web, Linux's refusal, and agent `tree`/`layout` for it.

### ccheever — 2026-10-08T08:07:44Z

**Decision: Add scoped app:/ iframe loading first; defer a separate PDF element.**

Keep open with the bounded scope below.

Local app files should use the existing typed-document path and report failure. A new object/PDFKit surface is a larger addition; Chrome-specific viewer fragments are not portable PDF layout semantics.

Resolve only permitted app roots, preserve iframe sandbox/origin policy, revoke object URLs and report missing files. Prove PDF pixels on iOS as well as tree state. Keep fitted PDF UI in a module until a separate consumer-driven design is selected.
