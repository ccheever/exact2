# @exact/text-detection on devices, by hand

What the simulator and the agent's fixture pictures cannot show: a photo the
camera took of a real page, with its tilt, shadow and curl. The package's
drives (`packages/barcode-detection/test/agent.mjs`, which reads the text
fixtures too) cover the fixture pictures on macOS, an iOS simulator and the
web. Each step says what to see; a step that shows something else is a
failure to report with a screenshot.

Have a printed book open to a page of prose.

## iPhone

Build: `bun host/apple/build.mjs shelf-apple --ios --device --run`.

1. Open a book in Shelf. Tap Add Quote from Photo, then take a photo of the
   page (or choose one you took). The quote sheet lists the page's lines in
   reading order, each with a switch, all on.
2. Turn off the lines outside the passage you want and tap Add: the book's
   notes end with the lines left on, joined by spaces in curly quotes.
3. A photo held sideways (the phone turned 90°): the lines read upright, in
   order, as in step 1.
4. A photo of something with no text (a wall): "No text found in this
   photo."
5. A page in another script (Japanese, Chinese, Korean, Russian): the lines
   in that script when the device's language list holds it (Settings →
   General → Language & Region); otherwise fewer or garbled lines, which is
   expected.

## Mac

Build: `bun host/apple/build.mjs shelf-apple --run`.

1. As steps 1 to 4, choosing a photo of a page from Finder (AirDrop one from
   the phone). The first read after installing a new build can take up to a
   minute while macOS compiles the recognizer for this Mac's Neural Engine;
   later reads take under a second.

## Android phone (stage 4, not built)

The Kotlin implementation over ML Kit is not built (LLP 1133.027 §12, stage
4's as-built note). When it lands: as the iPhone steps on a Pixel, plus a
Devanagari, Chinese, Japanese or Korean page, which needs that script's
model (`script` on `createTextDetector`).

## The web

No browser reads text (D18): Shelf shows no Add Quote from Photo in Chrome,
Safari or Firefox.
