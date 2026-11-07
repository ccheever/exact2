# Text styles and colors

## Rough

The gallery had three colored text samples. They did not cover the different paths used for decorations, shadows, vibrancy, mixed runs, color glyphs or paragraph bands. The machine’s registry cache also supplied `foldhash` and `hashbrown` simulator objects built for iOS 26 while the app links for iOS 17; cleaning those two packages in this checkout rebuilt compatible objects. The web verifier initially built into the app’s output directory and drove the checkout’s other directory; explicit `EXACT_APP_DIR` and `EXACT_WEB_DIST` made both use `target/web-dist`.

## Lean in

The text section now compares sizes, weights, italic, serif and monospace faces, spacing, ligatures, decorations, mixed-color runs, transparency, layer opacity, sharp and blurred shadows, vibrant system labels, HDR text and shadows, wrapping, truncation, justification, combining marks, Arabic, Hebrew, Devanagari, CJK and emoji. A button changes words, color, variable weight and width, paragraph lines and a number with tabular digits. A bounded scroller holds one tall paragraph.

Each existing CSS and platform color swatch also appears as text on white and dark backgrounds. Literal colors, including white on white and transparent text, are intentional specimens.

## Needed

Physical-device inspection of color fidelity, HDR luminance, cursive attachment and scrolling remains necessary. A simulator cannot establish the phone's frame rate or display luminance.

## Checkpoints

`contract build apps/ui-gallery/app.contract` succeeds. The authored browser interaction test passes and its semantic walks agree. The verifier reports intentional fixed-size and literal-color specimens, plus overlaps where text scrolls under the sticky range control; it is not a clean verifier run. The development-signed iPhone archive builds, passes strict code-signature verification, enables `get-task-allow` and has matching debug symbols. Native execution is blocked by CoreSimulator: “Failed to start launchd_sim: could not bind to session.” Native color fidelity, glyph rendering, the numeral-roll animation and scrolling remain unverified.
