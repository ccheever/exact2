# Code review: a declared variable face draws at its declared weight (Apple), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--cwd /tmp/bsky-vf-rv`. A first run without `--always-approve` returned only its preamble and was discarded.
- **Method:** one brief (sha256 `cb93ab1836a509b29cc75beaebf3623ff14bbed1d3045d10c6ef7684a5bb0cc5`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit 3c6afedba in a detached worktree. The author (Claude, for the Bluesky clone) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** No findings. Its note that Linux (`snap_weight`) shapes a declared face at the requested weight where the web and now Apple pin it to the declared one is pre-existing and left for its own change (QUEUE).

I'll review this commit read-only against the web host, LLP 1019, and the CoreText registration path. First I'll read the review guidance and the commit itself.The request is a read-only review of HEAD, with a verdict against the web host and LLP 1019. I'll inspect the commit and the font-registration paths it touches.The diff is small: variation is applied at face registration. Next I’ll compare that with the web host, LLP 1019, and the Apple cascade that consumes the descriptor.The registration change is small; the risk is whether CoreText keeps that `wght` value through the cascade. I'll check the font's axes and that path directly.No findings.

The change is the right rule, applied at the right point. The web host emits one `@font-face` per declared face with a single `font-weight` (`host/web-js/src/faces.rs:38-44`). CSS Fonts 4 sets the `wght` axis from that descriptor: a single number pins the axis to that number, and only a range descriptor lets the computed weight drive the axis. LLP 1019 D6's "any value in range" is that descriptor range. The compiler stores one weight per face (the shorthand is weight 400), so the range is that one weight. `declaredFace` runs once, in `install`, where each file face's descriptor is stored (`Text.swift:645-646`). Matching in `matched` then selects among faces that already carry their declared weight, and every later `CTFont` — the base font, the cascade list in `font(size:weight:family:italic:)`, and Canvas text, which calls that same function — is created from it.

`cssWeight` stays on the system-font path (`Text.swift:744`), after the catalog return. That is the right split: a platform variable face has no declared weight, so the requested number goes on `wght`. A declared face is already pinned.

I checked the CoreText behavior on this Mac with `vendor/cosmic-text/fonts/InterVariable.ttf` and `InterVariable-Italic.ttf`:

- `CTFontManagerCreateFontDescriptorsFromData` and `FromURL` both return nine named instances; the first is Thin (`wght` 100). The descriptor carries only the PostScript name. `CTFontDescriptorCreateCopyWithVariation` still applies `wght` absolutely. Weight 600 matches the SemiBold instance glyph-for-glyph (advance difference 0). Weight 450 lands halfway between 400 and 500, so it does not snap to a named instance.
- Weight 400, the axis default, is omitted from `CTFontCopyVariation`, and the outlines are Regular (the "Hello" width moves from Thin's 36.5 to 38.2). The test's `?? 400` matches that.
- `opsz` follows the font size (14 at 14 pt, 32 at 32 pt), clamped to the axis. It does not stay on Thin's instance.
- The cascade built in `font(size:weight:family:italic:)` (`CTFontCopyFontDescriptor` plus `CTFontCreateCopyWithAttributes` with `kCTFontCascadeListAttribute`) keeps the weight, including after `CTFontManagerRegisterFontsForURL`. A feature-settings copy, as Canvas small-caps does, keeps it too.
- Clamp: declared 1000 draws at 900; declared 1 draws at 100. The face's match key stays the declared weight (`RegisteredFace.weight`), which is what a single `font-weight` descriptor matches as.
- Italic file: Thin Italic, slant −9.4°. After `declaredFace` at 400 and 600 the slant and the italic trait remain, and the width moves to Italic and SemiBold Italic. That file has `wght` and `opsz` only, so there is no `ital` axis to set.
- A static file (DejaVu Sans) has no `wght` axis; the guard returns the same descriptor, which is what the `===` assertion locks.
- A family that mixes a variable face and a static face still matches on the declared weights. The variable descriptor is not asked to cover the static face's weight.

Linux `snap_weight` (`host/linux/src/text/catalog.rs:269-286`) will shape at the requested weight when the matched file's `wght` axis covers it, so a face declared at 400 can draw at 600. The web emission does not do that. This commit follows the web host.

The test locks the helper: Thin is 100, 400 reads back as 400, 600 is 600, 1000 clamps to 900, and a static face is left as the same object. It does not call `font()` or load `InterVariable-Italic.ttf`. Both of those paths preserve the axis, as measured above. macOS is what ran; the same CoreText calls are what iOS 17 and tvOS 17 use.

READY
