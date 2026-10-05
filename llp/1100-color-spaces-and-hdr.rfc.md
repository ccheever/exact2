# LLP 1100: Color spaces and HDR — every space CSS names, the bits each picture needs

**Type:** RFC
**Status:** Implemented. What is not built is listed in §11.
**Systems:**
- Kernel: a color value that carries its space; the `dynamic-range-limit` row; two `exactViewport` fields
- `exact-color` (`color/`): CSS color parsing, CSS's color arithmetic, the table of standard spaces
- Contract: CSS Color 4/5 and CSS Color HDR syntax; `color-profile` declarations
- Raster (`host/raster`): decode variants, bytes per pixel, the fallback ladder
- Apple host: decode in the picture's own space at the source's precision, layer dynamic range, video, canvas, picker
- Web hosts (`host/web`, `host/web-js`): emit what was written; the browser is the oracle
- Linux host: sRGB only, because its platform has no color management
- Agent API: decode facts, `prefer` for display facts, extended-range samples
**Author:** Claude (Opus 5.5) for ide@expo.io
**Related:**
- LLP 1011 (images), LLP 1011.000 (animated images)
- LLP 1042 (video), LLP 1069.002 (media picker), LLP 1056 (Canvas 2D)
- LLP 1095 (platform colors)
- LLP 1034 (`light-dark()`), LLP 1066 (gradients), LLP 1062 (paint motion)
- LLP 1069.000 (device facts, `exactViewport`, `prefer`), LLP 1012 (agent API)
- LLP 1010 §6.3 (the 32 MiB raster budget), LLP 1047 (pay for what you use)
- LLP 1001 §1 (declared deviations)
- CSS Color 4, CSS Color 5 (`@color-profile`), CSS Color HDR 1 (`rec2100-*`, `dynamic-range-limit`), Media Queries 5 (`color-gamut`, `dynamic-range`, `video-dynamic-range`), HTML canvas (`colorSpace`, `colorType`), ISO 21496-1 (gain maps), ITU-R BT.2100 and BT.2408 (PQ, HLG, reference white), ITU-T H.273 (CICP)

## Summary

1. **A color names its space.** Every CSS Color 4 function and predefined
   space is admitted, plus CSS Color HDR's `rec2100-pq`, `rec2100-hlg` and
   `rec2100-linear`, and CSS Color 5's `color-profile` for any ICC profile.
   Other standard spaces the platforms ship (DCI-P3, BT.709, ACEScg and
   others) have standard dashed names. The platform does the color
   management. A space a target can't show is a compile-time error for that
   target (D1–D3).
2. **A picture keeps its own color space and its own precision.** At decode
   a picture is one of four classes:
   - **standard** (8-bit sRGB or untagged): 4 bytes per pixel.
   - **wide** (8-bit with another profile): 4 bytes per pixel, kept in its
     own space.
   - **deep** (more than 8 bits per channel, or float samples): 8 bytes per
     pixel, half float, in its own space, clipped to SDR white.
   - **HDR** (a gain map, PQ or HLG): 8 bytes per pixel reserved. It is
     shown as HDR when the display can and `dynamic-range-limit` allows it,
     and as ImageIO's SDR rendition otherwise.

   The budget charges real bytes. Only under budget pressure does a deep
   picture drop to 8 bits, and that is reported (D4–D7).
3. **`dynamic-range-limit` governs everything that can draw above SDR
   white:** pictures, video, HDR colors, text, shadows, SVG paint and the GPU
   canvas. The OS does color matching, tone mapping and headroom tracking.
   Exact never tone-maps and never converts to the display's space (D8–D11).
4. **Verification** uses known-value fixtures, the platform's own views, the
   browser, and a person looking at a real HDR panel (§8, §9).

## 1. Principles

- **The web is the standard.** Names, values, defaults and the meaning of
  untagged content follow CSS:
  - Untagged pixels and untagged colors are sRGB.
  - `dynamic-range-limit`'s initial value is CSS's, `no-limit`.
  - Interpolation follows CSS Color 4 §12.
- **Standards, not vendors.** A space is named by the standard that defines
  it: the CSS name where CSS has one, otherwise a dashed ITU, SMPTE, ISO or
  ACES name. Nothing author-facing is spelled `apple-`, `cg-`, `-webkit-` or
  `android-`.
- **The platform manages color. Exact does not.**
  - Profile conversion, matching to the display and tone mapping belong to
    Core Graphics, Core Animation and ImageIO on Apple, and to the browser on
    the web.
  - Exact does only the arithmetic CSS defines for CSS values: reading
    `oklch()` into components, gradient interpolation, and motion.
  - Exact parses no ICC profile tables and links no color-management
    library.
  - Where a platform can't display a space, that space is not available on
    that platform.
- **The OS decides how the display is driven.** Exact tags what it hands the
  compositor with its space and headroom, and asks for a dynamic range. It
  does not convert to the display's profile, does not read the current EDR
  headroom, and does not tone-map.
- **Pay for what you use** (LLP 1047). An 8-bit sRGB picture costs 4 bytes a
  pixel, as before. A picture's bits follow its source. The core pays
  nothing for a capability an app doesn't use.
- **Deterministic under the agent.** The display facts that change a
  decision (gamut, dynamic range) can be pinned with `prefer`.

## 2. Decisions: color values

### D1 — One table of spaces, named by standards

The spaces live in `exact-color` (`color/src/lib.rs`): CSS's predefined
spaces, and `exact_color::PROFILES` for the dashed standard spaces with each
one's Apple name and component count.

The spaces, by source:

- **CSS Color 4 predefined spaces:**
  - `srgb`, `srgb-linear`, `display-p3`, `display-p3-linear`
  - `a98-rgb`, `prophoto-rgb`, `rec2020`
  - `xyz`, `xyz-d50`, `xyz-d65`
- **CSS Color 4 functions:** `rgb()`, `hsl()`, `hwb()`, `lab()`, `lch()`,
  `oklab()`, `oklch()`, the 148 named colors, `transparent`,
  `currentcolor`, and the system colors (LLP 1095 D2).
- **CSS Color HDR 1:** `rec2100-pq`, `rec2100-hlg`, `rec2100-linear`.
- **Standard spaces the platforms ship.** These are profiles Exact declares
  for every app, so they use CSS Color 5's dashed profile names:

  | name | standard | Apple |
  |---|---|---|
  | `--dci-p3` | SMPTE RP 431-2 | `kCGColorSpaceDCIP3` |
  | `--rec709` | ITU-R BT.709 / BT.1886 | `kCGColorSpaceITUR_709` |
  | `--rec2020-srgb-transfer` | BT.2020 primaries with the sRGB curve | `kCGColorSpaceITUR_2020_sRGBGamma` |
  | `--rec2020-linear` | BT.2020, linear | `kCGColorSpaceLinearITUR_2020` |
  | `--display-p3-pq`, `--display-p3-hlg` | P3 primaries with BT.2100 transfers | `kCGColorSpaceDisplayP3_PQ`, `…_HLG` |
  | `--rec709-pq`, `--rec709-hlg` | | `kCGColorSpaceITUR_709_PQ`, `…_HLG` |
  | `--aces-cg` | ACES AP1, linear (S-2014-004) | `kCGColorSpaceACESCGLinear` |
  | `--romm-rgb` | ISO 22028-2 | `kCGColorSpaceROMMRGB` (alias of `prophoto-rgb`) |
  | `--gray-gamma-2.2`, `--gray-linear` | | `kCGColorSpaceGenericGrayGamma2_2`, `kCGColorSpaceLinearGray` |

  If CSS later predefines one of these spaces, the CSS name replaces the
  dashed one, with no alias kept.
- **Any other space:** an ICC profile, through `color-profile` (D3).

Vendor-prefixed names (`apple-dci-p3`) are not used. Undashed names for
non-CSS spaces are not used, because they could collide with future CSS
names.

**Availability per platform:**

| platform | shows |
|---|---|
| Apple | every space above and every ICC profile Core Graphics accepts |
| web | every CSS space except `rec2100-*` (no browser draws them); no dashed space or ICC profile |
| Linux | colors inside the sRGB gamut, whatever function wrote them |

- **At build** (`exact_bake::colors`): a style row bound to a *literal*
  color the platform can't show is refused. The message names the color, the
  platform and the way out. This covers gradient and shadow colors and named
  styles' rows.
- A row *computed* by an expression is not refused at build. Choosing a wide
  color where `exactViewport().colorGamut` says the display has it is the
  intended pattern.
- **At run time** (`style::wide::set_available`, set by the Linux and Rust web hosts at
  boot): a color the host can't show, set dynamically, is refused as an
  invalid color is. The row takes its initial value.
- An app that wants a different color per platform uses LLP 1095 D3's
  `platform-color()`, or a CSS space every target has.
- Exact never converts a color into another space to make it available
  somewhere.

### D2 — The color value carries its space

`ColorValue` (`kernel/src/style.rs`) has these forms for colors in a space:

| form | holds | wire tag |
|---|---|---|
| `Fixed(Color)` | an 8-bit sRGB color | 0 |
| `Wide(u16)` | a color in its own space, or a `light-dark()` with one such half; an id into `style::wide` | 4, then the canonical CSS text |
| `Moving([i16; 3], u8)` | a paint-motion frame: extended linear sRGB in 1/2048ths, 8-bit alpha; never authored, never interned | 5 |
| `Profiled(u16)` | a color in a dashed space or an ICC profile; an id into `style::profiled` | 6, then the CSS text |

- The legacy sRGB forms (hex, `rgb()`, `hsl()`, `hwb()`, named colors) are
  `Fixed`.
- `lab()`, `lch()`, `oklab()`, `oklch()` and `color()` are `Wide`.
- `Wide` colors are interned up to 4096; `Profiled` colors up to 1024.
  Overflow is a refusal (`None`), never an sRGB clip. Per-runner ownership
  remains unbuilt (§11).
- A `Wide` color's fallback, for a reader that needs sRGB, is its sRGB clip.
  A `Profiled` color has no fallback: such a reader gets transparent, and on
  Apple Core Graphics converts it.

**The kernel does only CSS's arithmetic** (`exact-color`):

- Gradients take `in <space> [<method> hue]`, every CSS Color 4 §12 space
  and hue method, with premultiplied alpha. Without `in`, a gradient with a
  modern stop interpolates in Oklab; a gradient of legacy colors uses the
  8-bit sRGB path.
- A gradient that is not legacy is sampled sixteen times per stretch in
  extended linear sRGB. Apple carries a separate space flag for each
  appearance (`space`, `darkSpace`): a legacy half retains the byte sRGB
  ramp, which the presenter dense-samples in sRGB.
- A transition with an Oklab endpoint moves in Oklab when both endpoints
  fit `Moving` (linear sRGB components −16 through 15.999). Larger or
  non-finite converted endpoints change discretely, including shadows; the
  authored color remains admitted. A transition between
  two legacy colors moves in premultiplied sRGB. A finished transition
  presents its target in its own form.
- A computation over a `Profiled` color is refused: a gradient refuses it,
  and a transition to or from it is discrete.
- ICC declarations belong to the compiler/plan currently parsing styles. The
  parser binds them in a thread-local scope and restores the previous scope
  on return or unwinding. Preparing, discarding or accepting a candidate never
  mutates a live plan's declarations; an accepted plan's next parse uses its
  own complete declaration table (including removals).

**Hosts:**

- **Apple** makes each color in its own space. A wide color crosses as
  `{"cs": [{"s", "v"}…], "c": <sRGB clip>}`. `display-p3` and `srgb` cross
  as written. Any other CSS space crosses as extended linear sRGB, unclipped.
  Backgrounds, borders, text, inline runs, box shadows, text shadows, SVG
  fills and strokes, gradients and motion frames all draw in the color's
  space.
- **Web** emits the canonical CSS text (`color(display-p3 1 0 0)`), and the
  browser draws it.
- **Linux** draws sRGB-gamut colors. CSS's arithmetic gives the exact sRGB
  components of a `display-p3` color inside sRGB's gamut.

**An HDR color on Apple is tagged with its own exposure.** This is UIKit's
`linearExposure` model.

- A color in an extended-range or ITU-R 2100 (PQ, HLG) space is tagged with
  its own peak over SDR white, `max(1, peak)`, through `CGColor(headroom:)`
  (`ColorRange.tagged` in `DynamicRange.swift`).
- The peak is measured by Core Graphics' conversion of the color to extended
  linear Rec. 2020. Past 1 means brighter than white, not wider than sRGB.
- The peak is never read off the color itself, because Core Graphics
  answers 1000 nits for any untagged PQ or HLG color.
- The tag is the color's own exposure. The display's headroom stays the
  platform's.
- Tagging needs iOS 26 / macOS 26 / tvOS 26. Before 26 a color is not tagged.

Widening every color to `[f32; 4]` was rejected: it doubles the style
column for every app to serve the few colors that need it.

### D3 — `color-profile`: any ICC profile, as CSS Color 5 defines it

- **Contract** spells CSS's `@color-profile` as a top-level declaration
  without the `@`, with CSS's descriptors as attributes, as `keyframes` is
  `@keyframes`:

  ```
  color-profile --brand-press src="assets/swop.icc" rendering-intent="relative-colorimetric"
  ```

  It is used as CSS uses it: `color(--brand-press 0.1 0.8 0.2 0.05)`.
- Duplicate names, unknown descriptors, a missing `src` and an unknown
  intent are refused. The declarations lower into the plan's `profiles`
  table (name, src, intent).
- **The build checks** that each file exists, is at most 256 KiB, and is an
  ICC profile. It reads only the header's data color space, for the channel
  count. Each literal `color(--name …)` must give that many components.
  Exact never interprets a profile's tables.
- **Rendering intent** follows CSS Color 5: `relative-colorimetric` by
  default.
- **Apple** crosses a profiled color as `{"cs": [{"s": "cg:<name>" |
  "icc:<asset>", "i": <intent>, "v": [components…, alpha]}]}`.
  `ProfileSpaces` makes the space with `CGColorSpace(name:)` or
  `CGColorSpace(iccData:)` from the owning runtime's asset resolver. The batch
  decoder binds ICC paths to SHA-256 content identities before any view or
  text worker reads them, including explicit-plan boot. A bounded cache holds
  the last 64 distinct profiles; decoded values retain immutable profile handles
  across eviction, file replacement and session/generation changes. Unused
  evicted handles are reclaimed. A cache miss resolves the owning asset again.
  ICC colors are converted once to extended sRGB with the authored rendering
  intent, since `CGColor` itself does not retain an intent.
- **Web and Linux** don't have it. The web gets it when browsers ship
  `@color-profile`, and then emits the at-rule as written.
- An image's own embedded profile needs no declaration; the decoder reads
  it (D4).

## 3. Decisions: images

### D4 — Classify at decode: standard, wide, deep, HDR

`RasterMetadata` (`host/apple/Sources/ExactKit/RasterImage.swift`) reads,
from the header prefix only:

- `kCGImagePropertyDepth` and `kCGImagePropertyIsFloat`
- the transfer, from the lazily created image's color space
  (`CGColorSpaceIsPQBased`, `CGColorSpaceIsHLGBased`). This is how a
  CICP-tagged PNG or AVIF is recognized.
- a gain map: `kCGImageAuxiliaryDataTypeHDRGainMap` or
  `kCGImageAuxiliaryDataTypeISOGainMap` (iOS 18 / macOS 15) when ImageIO
  sees it in the prefix; otherwise the gain map's metadata in the JPEG's
  first segments (ISO 21496-1's URN, `hdrgm:Version`, Apple's HDRGainMap
  namespace).

When ImageIO reads no properties from a partial JPEG (a gain-map JPEG
larger than the 256 KiB header limit), `jpegHeader` reads size, precision
and EXIF orientation from the JPEG markers.

| class | what puts a picture in it |
|---|---|
| **standard** | 8 bits or fewer per channel; sRGB, untagged, gray, palette GIF |
| **wide** | 8 bits or fewer per channel; any other RGB or CMYK profile: Display P3, Adobe RGB, rec2020, DCI-P3, a camera or scanner ICC |
| **deep** | more than 8 bits per channel (10-, 12- and 16-bit PNG, TIFF, HEIC, AVIF, JPEG XL), or float samples (OpenEXR); SDR; any profile, sRGB included |
| **HDR** | a gain map, or a PQ or HLG transfer |

Float samples are deep, not HDR. `CGImageContainsImageSpecificToneMappingMetadata`
is not read.

### D5 — Storage per class: bits follow the source

| class | display / limit | stored as | B/px |
|---|---|---|---|
| standard | any | 8-bit, sRGB | 4 |
| wide, RGB | any | the source's 8-bit pixels in the source's own space, adopted without a redraw when the layout allows (D6) | 4 |
| wide, gray | any | 8-bit sRGB | 4 |
| wide, CMYK or Lab | any | 8-bit, as ImageIO's thumbnail converts it (sRGB for CMYK); anything else that is not RGB is drawn into Display P3 | 4 |
| deep | any | RGBA16F premultiplied, in the **standard-range** form of the source's space (sRGB, Display P3, rec2020, linear sRGB…). Values above SDR white clip to white | 8 |
| HDR | HDR display, limit `constrained` or `no-limit` | `kCGImageSourceDecodeToHDR`. A decode ImageIO returns in a BT.2100 space (PQ, HLG, a gain map's HDR rendition) is kept as ImageIO decoded it, with ImageIO's headroom. Any other HDR decode is drawn into extended linear Display P3 at half float. The image is tagged with its headroom (`CGImageCreateCopyWithContentHeadroom`) | 8 reserved |
| HDR | SDR display, or limit `standard` | `kCGImageSourceDecodeToSDR`: ImageIO's SDR rendition at the depth it yields | 4 or 8 |

- **Deep pictures keep their bits.** A 10- or 16-bit picture was made with
  that precision, usually for smooth gradients. Core Animation's contents
  come only in RGBA8, RGBA16F and Gray8, so "more than 8 bits" means
  RGBA16F. Half float holds 10–16-bit sources without visible loss.
- **Deep pictures keep precision, not light.** A deep picture is not marked
  HDR, so a value above 1 would show bright or dim as the OS chooses. The
  standard-range form clips it instead.
- **A BT.2100 decode is kept as ImageIO decoded it** (10 bits packed, or 16
  for a 16-bit PNG). Core Animation tone-maps BT.2100 contents to the
  layer's range itself. It does not do that for extended linear contents.
- **The SDR rendition's depth follows the container**, not the header: 8-bit
  Display P3 from a 10-bit HEIC or AVIF, 16-bit from a 16-bit PNG. So the
  plan reserves by the header's depth: 8 bytes a pixel for any source above
  8 bits.
- **CMYK** comes back from ImageIO's thumbnail as 8-bit sRGB. That is the
  platform's conversion. Exact adds none.
- **ImageIO reports an untagged picture as sRGB**, as CSS says.
- **The default PQ/HLG headroom** is 1000/203 ≈ 4.93: BT.2408's 203 cd/m²
  reference white is 1.0.
- **Options:** `kCGImageSourceShouldAllowFloat` is true for deep plans only.
  HDR passes `kCGImageSourceDecodeRequest`.
  `kCGImageSourceGenerateImageSpecificLumaScaling` stays at its default.
- **Orientation** is applied through
  `kCGImageSourceCreateThumbnailWithTransform`.
- **Animated images:** GIF is standard. Animated HDR sequences (AVIF) are
  not played (§10).

**Cost.** A 16-bit PNG costs 2× an 8-bit one. An app that doesn't want that
ships 8-bit assets. The agent's image facts show each picture's bytes per
pixel.

### D6 — The no-copy path admits any RGB space

`isAdoptable` keeps ImageIO's decoded image without a redraw when it is
already at the planned size, 8-bit, opaque, in the storage space, in a
layout Core Animation shares. Any RGB space qualifies, so a Display P3 or
Adobe RGB photo is not redrawn.

When a redraw is needed (alpha, size, layout), `normalized` draws into a
context of the picture's storage space: 8-bit for standard and wide, 16-bit
float for deep and for HDR decodes that are not BT.2100.

Core Animation color-matches tagged contents on both platforms. Exact does
nothing per display.

### D7 — The budget charges real bytes

- **`RasterDecodePlan`** carries bytes per pixel (4 or 8). Stride, output
  bytes and the peak follow from it.
- **`host/raster`'s admission** checks the stride against the variant's
  bytes per pixel and refuses an unknown variant. `SESSION_BYTES` (32 MiB)
  is unchanged. The Apple C ABI's `ExactRasterDemand` and `ExactRasterWork`
  carry the variant.
- **`RasterKey.variant`** (`host/raster/src/types.rs`, `variant::*`):

  | value | name | meaning |
  |---|---|---|
  | 1 | `SRGB8` | 8-bit sRGB. Linux's only variant |
  | 2 | `OWN8` | 8-bit in the picture's own space. Apple's standard and wide pictures, and an 8-bit SDR rendition |
  | 3 | `DEEP` | RGBA16F in the picture's own space, including a deep SDR rendition |
  | 4 | `HDR` | an HDR decode, 8 bytes a pixel reserved |
  | 5 | `REDUCED8` | the budget's fallback: 8 bits in the picture's own space |

- **The fallback ladder** (`RasterLoader.fit`). Dropping bits is the last
  resort, never a default:
  1. An HDR plan, if the picture is HDR and HDR is wanted.
  2. The picture's own SDR variant (`OWN8` or `DEEP`).
  3. For a deep plan, `REDUCED8` at the same pixel size.
  4. Only then is resolution reduced.

  A `REDUCED8` decode is reported as `fallback: "budget"` (D12). A later
  decode with room takes the full variant again.
- **The Swift budget** (`RasterLoader.viewportBudget`, eight viewport-sized
  bitmaps at 4 B/px, clamped to 32–192 MiB) stays in bytes. A deep or HDR
  bitmap charges its 8.

### D7a — Gain-map compositing at 8 bits is not done

Composing a gain-map picture at draw time from its 8-bit base and gain map
would hold an HDR photo in about 1.1–2× SDR bytes instead of 2×. Core
Animation can't apply a gain map to `contents`, so it would need a Metal or
Core Image layer per picture and Exact's own ISO 21496-1 math. That is
color work the platform already does. Exact does not do it. It is
reconsidered only if V7 shows HDR pictures holding more than a quarter of
the raster budget in a real app. Variant `6` is left free for it.

## 4. Decisions: dynamic range, display facts, video, canvas

### D8 — `dynamic-range-limit`, as CSS Color HDR defines it

The style row `dynamic_range_limit` (schema bit 179):
- values `standard | constrained | no-limit`
- **inherited**, initial `no-limit`
- a change is paint, not layout
- Contract accepts it on any element

`dynamic-range-limit-mix()` is not supported.

**The default is the same on every platform: CSS's `no-limit`.** An HDR
picture in an `image` and an HDR clip in a `video` are both HDR unless the
app sets the row. Exact does not take each Apple view's own default (a bare
`CALayer` is `.standard`; AVKit shows HDR), because a photo would then be
SDR beside a glowing video, and the app would differ from its web build.

- **Switching it** is one inherited row. `dynamic-range-limit="standard"` on
  the root makes the whole app SDR. On a subtree it makes only that subtree
  SDR. `constrained` is HDR kept close to SDR white.
- The value can be bound to state, so a setting can switch it live.
- The OS has the last word. Low Power Mode, a backgrounded scene and
  macOS's HDR suppression lower the headroom. `no-limit` is a request.

**Apple, iOS / macOS / tvOS 26 and later.** Every layer a node paints something
above SDR white on gets `preferredDynamicRange` from the node's inherited
limit:

| CSS value | `CALayer.DynamicRange` |
|---|---|
| `standard` | `.standard` |
| `constrained` | `.constrainedHigh` |
| `no-limit` | `.high` |

These layers are:
- an image layer holding an HDR (variant `4`) bitmap, which also gets
  `contentsHeadroom` from the decode
- the layer of a background, a border, a box fill, a box shadow's casters,
  and an SVG scene's shapes, when any color it paints is HDR
  (`NodeView.applyColorRanges`, on each box paint and each limit change)
- a text raster's layer, when its ink or its `text-shadow` is HDR, or its
  wide SDR ink needs extended storage outside Display P3. Headroom 1 marks
  the latter; macOS retains it on the IOSurface and configures both direct
  and overflow layers, UIKit retains it on the CGImage and ink layer
- a view's own layer, when what it draws itself in `draw(_:)` is HDR (below)
- an inline `AVPlayerLayer` (D11)
- an HDR GPU canvas's layer (D12b)

Rules that follow:
- **Any other layer stays `.standard`.** Wide SDR colors needing extended
  components also request the node's range, with headroom 1. An EDR layer
  costs the compositor and battery even when its content is SDR.
- **An HDR fill is never a flat leaf.** A run of flat leaves shares one
  shape layer, which has one range.
- **`toneMapMode` is not set.** It stays Core Animation's automatic mode.
- **Exact does not tone-map colors or pictures itself.** Core Animation
  does.
- **The limit reaches every node.** The Apple host sends a box its
  inherited `dynamic_range_limit` with its other inherited rows.

**Apple, before 26:** `wantsExtendedDynamicRangeContent = true` for
`constrained` and `no-limit`. *Declared deviation:* that API has no
"constrained", so `constrained` draws as `no-limit`. tvOS has no such
fallback API; before tvOS 26 these layers stay standard range.

**Web:** emitted by name, as written. The browser applies it.
- *Declared deviation:* Mobile Safari 27.0 rejects `constrained`
  (`CSS.supports('dynamic-range-limit', 'constrained')` is false), so it
  drops the declaration. The node then inherits its parent's limit. The web
  host still emits `constrained` as written. The color gallery says so on
  Safari.
- `standard` and `no-limit` work on Safari.

**Linux:** SDR output (D10). The value is held but changes nothing.

**HDR text and a view's own drawing:**

- **Text raster.** A paragraph whose ink or shadow is HDR rasterizes at
  half float in extended sRGB (RGhA on macOS). SDR text keeps 4 bytes a
  pixel when its gamut fits sRGB or Display P3. Profiled ink outside P3
  uses half float in extended sRGB even below SDR white. The raster is tagged with the peak of its colors where Core
  Animation reads it:
  - iOS: `CGImageCreateCopyWithContentHeadroom`
  - macOS: `kIOSurfaceContentHeadroom` on the IOSurface
- **Text shadow.** `text-shadow` keeps its color's space. It crosses as the
  text's nine channels: the sRGB four, then the space and four components
  (`BatchValue.textChannels`). An HDR shadow puts its layer in the limit's
  range even over SDR ink.
- **A view's own drawing.** Some things a view draws itself in `draw(_:)`:
  a box the layer can't express (sides of two widths or colors, a radius or
  `corner-shape` the layer can't do), and a paragraph drawn rather than
  rastered (small text on macOS, a failed raster). When that drawing
  reaches past SDR white:
  - the backing store is `RGBA16Float`
  - the layer sets `contentsHeadroom` to the peak, because a backing store
    has no tag of its own
  - the layer asks for the limit's range

  Gradient stops participate too, both on `CAGradientLayer` and in conic or
  multi-layer drawing. Stops are tagged with their exposure. Extended RGB
  components outside [0,1] also require the extended layer and half-float
  backing, even when their Rec. 2020 headroom is 1 (wide SDR).
  The backing store returns to 8 bits when both light and gamut fit it.
- **Not HDR on Apple: SVG gradients, patterns, masks and filters.** Each is
  drawn into an 8-bit sRGB picture (`SvgPaint.gradient`, `SvgIsland`,
  `SvgFilterGPU`, `SvgFilterLive`), so a color past SDR white there clips to
  white under every limit. The color gallery says so under its HDR colors.

### D9 — Display facts: `colorGamut`, `dynamicRange`

`exactViewport` has two Media Queries 5 features (LLP 1069.000 D1's
pattern):
- `colorGamut`: `srgb | p3 | rec2020`
- `dynamicRange`: `standard | high`

`video-dynamic-range` is the same value on every host Exact has, so there
is no separate fact. The runner's `Preferences` carry them: bits 8–9 the
gamut (256 p3, 512 rec2020), bit 10 a high dynamic range.

**Sources:**

| fact | iOS | macOS | web | Linux |
|---|---|---|---|---|
| gamut | `UIScreen.main.traitCollection.displayGamut` | `view.window.screen.canRepresent(.p3)` | `matchMedia('(color-gamut: …)')` | `srgb` |
| dynamic range | `UIScreen.potentialEDRHeadroom > 1` | `maximumPotentialExtendedDynamicRangeColorComponentValue > 1` | `matchMedia('(dynamic-range: high)')` | `standard` |

Neither Apple platform reports a rec2020 panel.

**What decides an HDR decode** (`DisplayRange.showsHDR`): the display's
*potential* headroom is above 1, and the node's limit is not `standard`.

- The current headroom is never used. It moves with ambient light and
  brightness, and the compositor tone-maps to it. A brightness change
  re-decodes nothing.
- OS suppression of HDR (iOS 26's `hdrHeadroomUsageLimit`, macOS 26's
  suppression notifications) does not re-decode a picture as SDR. The OS
  already limits an HDR layer's headroom while it suppresses.
- These re-check every HDR picture's plan: a window changing screens or the
  screen parameters changing (macOS; these also republish the session's
  viewport display facts from its own screen), the suppression notifications (macOS
  26), the `hdrHeadroomUsageLimit` trait changing (iOS 26), and a node's
  limit changing.
- Going to `standard` re-decodes the picture to its SDR rendition, which
  frees the 16-bit bitmap.

**`prefer`** pins `color-gamut` and `dynamic-range` on Apple and on the web
(CDP media emulation). Under the agent a display starts SDR and sRGB.
Linux accepts only `srgb` and `standard`.

### D10 — Linux: sRGB, because the platform has no color management

The Linux host is pure Rust. It draws XRGB8888 buffers through DRM/KMS.
Nothing below it does color management: no compositor, no color daemon, no
display profile. So Linux shows sRGB only. Exact does not bring a
color-management engine to make up for it.

- **CSS colors:** a color inside sRGB's gamut is drawn, whatever function
  wrote it. A literal color outside it is a compile-time error for Linux. A
  dynamic one is refused when set (D1). Profiled colors are refused.
- **Pictures:** the PNG decoder ignores `iCCP`. An untagged or sRGB picture
  is correct. A picture tagged with another profile has its pixels drawn as
  if they were sRGB. This is a declared deviation (LLP 1001 §1).
- **Depth:** a 16-bit picture is reduced to 8 bits, because Linux's output
  is 8-bit. This is the one place bits do not follow the source.
- **HDR pictures:** a gain-map JPEG's base image is a plain SDR picture and
  is drawn as one. A PQ or HLG picture is drawn as its raw pixels.
- **Canvas:** sRGB and 8-bit only. The build refuses a literal
  `display-p3` or `float16` canvas for Linux.
- **The color gallery** is not built for Linux.
- If a Linux display stack with color management becomes a target (a
  Wayland compositor with the color-management protocol, or DRM's
  `Colorspace` and `HDR_OUTPUT_METADATA` with 10-bit buffers), Linux takes
  its availability from that platform as Apple does.

### D11 — Video

Apple video already shows HDR and P3 through AVKit. Exact adds CSS's
control:

- **`dynamic-range-limit` on a `video`** (`VideoArm.applyDynamicRange`),
  with D8's mapping:
  - AVKit: `AVPlayerViewController.preferredDisplayDynamicRange` (iOS 26)
    or `AVPlayerView.preferredDisplayDynamicRange` (macOS 26).
  - The inline `AVPlayerLayer` (no controls): `preferredDynamicRange` on
    iOS 26, `wantsExtendedDynamicRangeContent` before.
  - *Declared deviation:* before 26, AVKit's controller has no range
    control. tvOS has no AVKit range preference.
- **The poster** is decoded by the platform: `UIImageReader` with
  `prefersHighDynamicRange` on iOS (gain maps kept), `NSImage` on macOS. The
  poster view's `preferredImageDynamicRange` follows the limit. The poster
  stays in the video artifact, not in ExactKit's raster pipeline, so it is
  not budgeted.
- **The agent's media state** (`state.media`) has:
  - `hdr`: whether the item's video track has `.containsHDRVideo`
  - `eligibleForHDR`: `AVPlayer.eligibleForHDRPlayback`
  - `dynamicRange`: the limit asked of the player
- **Web:** `<video>` and the row, by name. **Linux:** no playback.

### D12 — The agent sees decode facts and extended-range samples

An 8-bit sRGB screenshot can't check a color decision. `screenshot` stays
8-bit sRGB, the deterministic picture for people. Two additions:

- **Image facts.** Each entry of the raster diagnostics' `images` has
  `color`:

  ```json
  {"variant":"hdr","fallback":null,"space":"rec2100-pq","bitsPerComponent":16,
   "bytesPerPixel":8,"headroom":4.93,
   "layer":{"dynamicRange":"high","contentsHeadroom":4.93,"current":true}}
  ```

  - `variant`: `srgb8`, `own8`, `deep`, `hdr` or `reduced8`
  - `fallback`: `"budget"` for `reduced8`, else null
  - `space`: D1's names (`colorSpaceName`), never platform names
  - `headroom`: for an HDR bitmap only
- **`sample <x> <y> [<x> <y>…]`** returns extended linear sRGB values under
  view points. On Apple (macOS and iOS) the view's layers are rendered into
  a 32-bit float extended linear sRGB context. Values above 1 are HDR.
  Negative values are outside sRGB. This avoids 16-bit PNGs.

### D12a — Canvas 2D: `colorSpace` and `colorType`, as HTML defines them

- **Contract:** a canvas takes `color-space` (`srgb` | `display-p3`) and
  `color-type` (`unorm8` | `float16`) as attributes, HTML's `getContext`
  names (props `colorSpace` 240 and `colorType` 241), since app code never
  calls `getContext`.
- A change is a new generation, as a new context would be.
- `float16` counts 8 bytes a pixel against the canvas budget.
- **The recorders** (Rust and TypeScript, byte for byte): in a `display-p3`
  canvas, a color's list bytes are Display P3, made from a wide color's own
  components and clipped to P3. One matrix, `LINEAR_SRGB_TO_P3`, the same
  digits in both. Getters return each color's own serialization.
- `createImageData` takes the canvas's space. `ImageData` has its
  `colorSpace`. `putImageData` converts pixels between spaces as HTML does.
- `drawImage` of a wide picture into an sRGB canvas clips, as the browser
  does.
- **Apple:** colors, gradients, conic shading, offscreen composites and put
  pixels are in the canvas's space. A `float16` bitmap is RGBA half floats
  in the extended space. The GPU canvas's IOSurface is tagged Display P3,
  and its images are converted to it. A `float16` canvas always uses Core
  Graphics, because the GPU module's surfaces are 8-bit.
- **Web:** both glues pass the settings to the real `getContext`, give a P3
  canvas its colors as `color(display-p3 …)` and its pixels as P3
  `ImageData`, and replace the element when the settings change (an element
  keeps its first context's settings).
- **Linux:** sRGB, 8-bit (D10).
- `state.canvas` reports `colorSpace` and `colorType`.

### D12b — The GPU module and the game: an opt-in HDR surface

- **The surface** opts in with `Surface::high_dynamic_range()` (default
  `false`), asked once at create. Where the target supports it, the module
  configures `Rgba16Float` with wgpu's `SurfaceColorSpace::ExtendedSrgb`.
  wgpu sets `wantsExtendedDynamicRangeContent` and the extended color space
  on the `CAMetalLayer`, and extended tone mapping on the web.
- **The encoding is sRGB-encoded extended**, not linear: the 8-bit target's
  encoding continued past 1. One shader serves both, and the web, which has
  no linear canvas space, matches.
- `EDRMetadata` is not set. Core Animation tone-maps to the display.
- **`Frame::headroom`** is how far above SDR white the frame may draw. It is
  1 for a surface that did not ask or got no HDR target. The host provides
  it through `gpu_headroom` and `gpu_high_dynamic_range` (optional exports,
  so an older module still loads):
  - Apple: the display's potential headroom as the node's limit allows (1
    under `standard` or on an SDR display). The layer's
    `preferredDynamicRange` follows the limit (26 and later).
  - Web: there is no headroom query. A `(dynamic-range: high)` display is
    given 4, or 2 under `constrained`. *Declared:* this is a guess; the
    browser clips at the panel's own.
- **The game** opts a world in with `Game::HIGH_DYNAMIC_RANGE`. The tone
  pass maps white to the headroom: `h·f(x/h)` with Narkowicz's ACES fit. At
  `h = 1` it is the SDR curve.

### D13 — The picker keeps what the camera made

LLP 1069.002's HEIC → JPEG conversion (`Picker.jpeg`) writes JPEG at
quality 0.9 with orientation applied and no metadata copied.

- The color profile is kept.
- From iOS 18 / macOS 15, an HDR photo (a gain map, PQ or HLG) is decoded
  with `kCGImageSourceDecodeToHDR` and written with
  `kCGImageDestinationEncodeToISOGainmap`, so it stays HDR.
- Before 18, an HDR photo becomes its SDR picture.

## 5. What this changes elsewhere

| document | change |
|---|---|
| LLP 1011 §1, §4 | "normalized sRGB RGBA8" is replaced by D4–D7 |
| LLP 1042 | D11 |
| LLP 1056 | the `colorSpace` row is D12a |
| LLP 1069.000 | two `exactViewport` fields; `prefer` keys |
| LLP 1069.002 | D13 |
| LLP 1095 | its wide color value and image color spaces are D2 and D4–D7 here; dynamic platform colors and device RGB fills stay with LLP 1095 |
| `kernel/tables/schema.json` | `dynamic_range_limit` (bit 179); props `colorSpace` (240), `colorType` (241) |

## 6. Costs

- **sRGB apps:** an 8-bit sRGB picture is 4 bytes a pixel and a legacy
  color is `Fixed(u32)`, as before.
- **Web core:** the parser takes CSS Color 4's functions and the named
  colors. The web emits colors as text.
- **Apple:** the HDR decode path and the layer ranges are in ExactKit. The
  video changes are in the video artifact. No new artifact.
- **Linux:** no new dependency.
- **Battery:** an EDR layer exists only where something HDR is on screen.
- **Motion:** a motion slot is 48 bytes.

## 7. Rejected alternatives

- **Convert every picture to 16-bit extended sRGB.** It doubles every wide
  picture's bytes. Wide pictures stay 8-bit in their own space.
- **Convert to the display's space at decode.** It is wrong when a window
  moves between screens and duplicates the compositor's work.
- **10-bit `bgr10_xr` for HDR.** Its range tops out near 1.25× SDR white.
  Core Animation's layer contents are only RGBA8, RGBA16F and Gray8.
- **`UIImageView` / `NSImageView` for HDR pictures.** It gives up the raster
  budget, the shared decode cache, `object-fit`, rounded clipping and the
  off-main decode. The native views are the test oracle instead (V3).
- **An Exact-only `hdr=` attribute.** CSS has `dynamic-range-limit`.
- **`standard` as the default.** The web build would differ from the
  browser's own behavior.

## 8. Verification

PR #94 regressions cover wide shadow targets (including discrete profiles),
wide-table refusal, selected `platform-color()` branches, the web's dynamic
refusals, recorder byte parity for Rec. 2020/Adobe RGB/ProPhoto, equal hues and
resolved `light-dark()` interpolation, and wide inset shading. Apple tests
cover real gradient layers and drawn box stores, profile replacement between
sessions/generations, authored ICC intents against Core Graphics, profiled
text storage, unknown PQ/HLG headroom, screen-change publication and HDR
image flight layers. Round 2 also covers role/platform inset fallbacks,
per-appearance gradient encoding and sampling (opaque and translucent),
explicit-plan ICC boot through a real session, wide text presentation layers,
candidate profile isolation through prepare/discard/commit, discrete large-HDR
transitions, and bounded ICC caching with live values surviving eviction.
The suggested native `platform-color` profile-fallback bake bypass was rejected:
`parse_platform` admits only plain legacy colors or a legacy `light-dark` pair;
profile fallbacks fail compilation before bake validation. Build/run results
are reported by the fix round; these regressions do not replace V6's physical-panel verification.

The five checks keep their 60 s. Anything on a simulator, browser or device
runs in the async lane or by a person.

### V1 — Kernel and colors (blocking, `cargo test`)

- **`exact-color`:** each matrix is derived from the standard's primaries;
  every interpolation space round-trips; each hue method goes the specified
  way; red to transparent stays red at half alpha; Oklab's midpoint is
  lighter than sRGB's.
- **Parsing and serialization:** the CSS Color 4 / 5 / HDR WPT
  color-parsing cases, vendored under `kernel/tests/fixtures/css-color/` for
  the subset Exact admits, parse to the specified value, refuse what CSS
  refuses, and serialize as CSS Color 4 §15 says. The 148 named colors
  round-trip.
- **Storage:** an 8-bit sRGB value is `Fixed`, a `display-p3` value is
  `Wide`; the wire round-trips tags 4, 5 and 6, and a `light-dark()` pair
  with a modern half.
- **Availability:** a literal color a target can't show is refused for that
  target at build and accepted for the others; a dynamic one is refused when
  set (`exact-bake`, `exact-linux`).
- **Profiles:** `color-profile` declarations and their colors
  (`contract/cli/tests/it/color_spaces.rs`, `exact-bake`).
- **`dynamic-range-limit`:** parsed, inherited, refused on a typo, initial
  `no-limit`.
- **Motion:** a modern color moves in Oklab.

### V2 — Raster policy (blocking, `cargo test -p exact-raster`)

- An 8 B/px plan is admitted when it fits and refused with the exact reason
  when it doesn't.
- `DecodeCost` and the session charge use real bytes per pixel.
- Variants are distinct keys. An unknown variant is refused.
- The ladder never refuses while a smaller plan would fit, and never reduces
  a plan that fits.

### V3 — Apple decode and paint (async lane, `build.mjs --test` and `--test --ios`)

`RasterColorTests`, `HDRImageTests`, `WideColorTests`, `ProfileColorTests`,
`PickerColorTests`, over the fixtures in §9:

1. **Class and storage.** Class, variant, stored space (by D1 name), bits
   per component, bytes per pixel and headroom match each fixture's JSON.
2. **Numbers.** Each fixture's patches, read back and converted to extended
   linear sRGB by test code over the standards' numbers, match the
   generator's values. Tolerances:
   - 1.5/255 for 8-bit
   - 1e-3 relative for 16F
   - 6% for PQ HEIC and AVIF patches and 4% for the PQ PNG, in cd/m² ÷ 203
3. **Adoption.** An opaque 8-bit P3 picture is adopted with no redraw.
4. **Depth.** `F6` (16-bit P3 ramp) keeps all 1024 steps distinct; an 8-bit
   store keeps fewer than 300. A 16-bit sRGB PNG is deep.
5. **HDR.** Gain maps recover light above white with headroom 4. SDR
   renditions stay at or below 1. Display and limit decide the variant.
6. **Platform parity.** Each fixture drawn by Exact's image layer and by a
   `UIImageView`/`NSImageView` with `preferredImageDynamicRange = .high`,
   both rendered into the same extended-range context, agree within mean ≤
   1/255 and ≤ 0.5% of pixels beyond 4/255.
7. **Layer ranges.** An HDR bitmap's layer, and a layer painting an HDR
   color, has the mapped `preferredDynamicRange` and its headroom. An SDR
   layer is `.standard`. A live limit change updates the range; going to
   `standard` re-plans to the SDR rendition.
8. **Display changes.** Under `prefer`, high → standard swaps to the SDR
   rendition and standard → high re-plans to HDR. A brightness change
   re-plans nothing.
9. **Colors.** A `color(display-p3 1 0 0)` box, a `--dci-p3` box, an ICC
   `color-profile` box and an `oklch()` gradient, read through `sample`,
   match the standard's numbers within 1/512 in extended linear sRGB. A wide
   paragraph rasters in Display P3. A transition to a wide color ends in its
   space.
10. **Canvas.** `display-p3` and `float16` contexts, `getImageData` color
    spaces, and `drawImage` clipping into an sRGB canvas.

### V4 — Video (async lane, iOS simulator and macOS)

- Clips `V1`–`V4` play, and `state.media` reports `hdr` correctly.
- `dynamic-range-limit` reaches the controller's or the layer's property on
  26 and later.
- The panel actually brightening can't be proven on a simulator; that is
  V6.

### V5 — The web oracle (async lane, headless Chrome)

The color gallery on the JS target. For each fixture `<img>`, the glue in
agent mode draws it into a `display-p3` canvas (and a `float16` one where
supported) and reads `getImageData`. The values must match the generator's
within V3.2's tolerances.

- Chrome is held to the same numbers, so a disagreement between Apple and
  the expected values is either Exact's bug or a documented difference
  between Chrome and ImageIO.
- The launch adds `--force-color-profile=srgb`, so 8-bit screenshots stay
  deterministic on a P3 machine.
- `getComputedStyle(img).dynamicRangeLimit`, and `matchMedia` under
  `prefer`.
- A feature the browser lacks is a declared web deviation. The web emits it
  anyway.

### V6 — On a real HDR panel (a person, recorded)

On an iPhone with an XDR display and a MacBook Pro with XDR:

1. **The seam test.** Each wide fixture sits beside a CSS box of its exact
   patch color, with no gap. A visible seam is a failure.
2. **HDR pictures.** `F11` shown by Exact, by a `UIImageView`, and by
   Photos/Preview: the bright patch glows equally. Under `standard` Exact's
   copy matches the SDR base. Under `constrained` it is visibly between the
   two.
3. **HDR colors.** `color(rec2100-linear 4 4 4)` on a background, a border,
   a shadow, an SVG shape and text steps down under each limit.
4. **Suppression.** Background the app on iOS 26 and return: HDR resumes.
   On macOS, move the window to an SDR display and back.
5. **Video.** `V2` (PQ) and `V3` (HLG) glow, and `standard` stops them
   (26 and later).
6. **Battery.** Instruments on a feed with no HDR content shows no EDR
   layer.

### V7 — Cost (async lane, `metrics.mjs --long` and the bench feeds)

- An sRGB photo feed: bytes and decode time per image match the baseline.
- A P3 camera photo feed: bytes match, and decode time falls (no redraw).
- An HDR feed (20 gain-map photos): 2× bytes on an HDR display, 1× under
  `prefer dynamic-range standard`, fling fps within noise of the SDR run,
  and the number of ladder fallbacks.
- A feed of 16-bit PNGs: 2× bytes, and `variant: deep` for each.

### V8 — Linux (blocking pinned tests, `host/linux/tests/pinned/`)

- `F1` and `F2` match Chrome's rendering of the same file (recorded into
  `scripts/fixtures/color/*.web.png` under `--force-color-profile=srgb`),
  within mean ≤ 2/255 and ≤ 2% beyond 48.
- A tagged picture (`F3`, `F12p`) draws its raw pixels.
- An sRGB-gamut `display-p3` color draws its CSS-computed sRGB value.
- A color outside sRGB is refused at build for Linux, and refused when set
  at run time.

## 9. Fixtures

**Where they live:** `scripts/fixtures/color/`, committed. Each picture is
small (64×64 to 256×64; `F17` is 4096×4096). Each has a sibling
`<name>.json` with its class, bits per component, and patch values in
extended linear sRGB (and in cd/m² for PQ). V3, V5 and V8 read the same
JSON.

**How they are made:** `scripts/fixtures/color/make.swift`, run by hand on
macOS 15 or later. It writes the pictures through ImageIO's own encoders
and the JSON from the same patch definitions. No check runs it.

- The PQ HEIC and AVIF are tagged with their 4000 cd/m² peak (headroom
  4000/203 ≈ 19.7), which ImageIO stores and reads back.
- The PQ PNG can't declare its peak: ImageIO reads no headroom from PNG (it
  ignores `cLLi` and `mDCV`). So `pq-cicp.png` takes PQ's default headroom,
  and its 4000 cd/m² patch shows at 1000. That is the platform reading the
  file.
- Gain maps are written with `kCGImageDestinationEncodeRequest` set to
  `kCGImageDestinationEncodeToISOGainmap` as an `AddImage` property, from an
  extended-range image tagged with `CGImageCreateCopyWithContentHeadroom`.

**The pictures:**

| id | file | class / expected | what it holds |
|---|---|---|---|
| F1 | `srgb-ramp.png` | standard | 8-bit sRGB ramp and primaries, `sRGB` chunk |
| F2 | `untagged.png` | standard (untagged is sRGB) | the same patches, no profile |
| F3 | `p3-primaries.png` | wide, adopted, 4 B/px | P3 red, green, blue, a P3 color inside sRGB, white, mid-gray |
| F4 | `p3-camera.jpg`, `p3.heic` | wide, adopted | F3's patches as a camera writes them |
| F5 | `p3-alpha.png` | wide, redrawn in Display P3 (alpha) | F3 at 50% alpha |
| F6 | `p3-16bit-ramp.png` | deep, RGBA16F in Display P3 | 1024 steps of P3 green, every step distinct |
| F6a | `srgb-16bit.png` | deep | F1's patches at 16 bits |
| F7 | `adobe-rgb.jpg` | wide | Adobe RGB primaries |
| F8 | `prophoto-16.tif` | deep, kept in ROMM RGB | a ProPhoto green outside P3 |
| F9 | `cmyk.jpg` | wide, stored as 8-bit sRGB (ImageIO's conversion) | SWOP CMYK patches |
| F10 | `gray-gamma22.png` | standard, stored as sRGB | a gray ICC ramp |
| F11 | `gainmap-iso.jpg` | HDR | ISO 21496-1: SDR white 1.0, a patch at 4.0, headroom 4 |
| F11a | `gainmap-iso.heic` | HDR | the same in HEIC |
| F12 | `pq.heic`, `pq.avif` | HDR, headroom 19.7 | 10-bit PQ neutrals at 0, 100, 203, 400, 1000, 4000 cd/m² |
| F12p | `pq-cicp.png` | HDR, headroom 4.93 (Apple); raw pixels (Linux) | 16-bit PNG with `cICP` 9/16/0, the same neutrals |
| F13 | `hlg.heic` | HDR | 10-bit HLG neutrals at 0, 25, 50, 75 (reference white) and 100% signal |
| F14 | `linear.exr` | deep, clipped to SDR white | linear 0.5, 1, 2, 8 |
| F15 | `anim.gif` | standard, animated | two frames, red then green |
| F16 | F11 under `tint-color` | standard | the tint path ignores HDR |
| F17 | `gainmap-huge.jpg` | HDR; a 16-bit HDR plan exceeds the 32 MiB ceiling, so it falls back to its SDR rendition | 4096×4096 |
| F18 | `gainmap-broken.jpg` | wide, the SDR base, no error | gain-map image cut off, metadata still names it |
| F19 | `profile-broken.jpg` | standard (an invalid profile is sRGB) | a corrupt ICC tag table |
| F20 | `p3-orient6.jpg`, `gainmap-orient6.heic` | wide; HDR | EXIF orientation 6 |

**The clips:**

| id | file | expected |
|---|---|---|
| V1 | `sdr.mp4` | H.264, BT.709 (control) |
| V2 | `pq.mov` | HEVC Main10, PQ, HDR10 metadata |
| V3 | `hlg.mov` | HEVC Main10, HLG |
| V4 | `p3-sdr.mov` | HEVC, P3 primaries, SDR |

**The color fixtures** (V1, V3.9, V5), one Contract file per family under
`scripts/fixtures/`:
- `color-functions.contract`: every CSS function and predefined space
- `color-profiles.contract`: every dashed space and an ICC `color-profile`
- `color-mix.contract`: interpolation spaces, gradients, motion
- `dynamic-range.contract`: the three limits over F11 and V2

**The color gallery** (`apps/color-gallery`) is one page under a sticky
`dynamic-range-limit` control:
- the CSS color spaces, the platform profiles and the image profiles side
  by side
- HDR pictures and HDR colors under the three limits
- canvases in both spaces and types
- P3 text and shadows

It is not built for Linux.

## 10. Not in this design

- **Color management on Linux.** The platform has none (D10).
- **Animated HDR** (AVIF sequences). LLP 1011.000 plays GIF and WebP only.
- **`image-set()` and `srcset` by `dynamic-range`.** These are LLP 1011
  §6's.
- **HDR in SVG gradients, patterns, masks and filters, and in Core Image
  backdrops.** These draw in SDR on Apple (D8; `Backdrop.swift`).
- **Encoding edited HDR pictures.** That belongs to the app that saves.
- **Gain-map compositing at 8 bits** (D7a).
- **`dynamic-range-limit-mix()`.**
- **Android.** There is no host. The design carries over: `ColorSpace.Named`
  (`DISPLAY_P3`, `BT2020_PQ`, `BT2020_HLG`) and
  `Window.setColorMode(COLOR_MODE_HDR)` (LLP 1076).

## 11. Not built

- **Runner ownership of interned color tables:** `WIDE` and `PROFILED`
  remain process-global, immutable after insertion, with shared capacity
  (4096 and 1024 respectively). Overflow refuses, never clips. Candidate
  preparation can still consume slots even when discarded; values are not
  reclaimed. Declared profile names are now isolated per compiler/plan parse,
  and interned profile values retain their resolved source and intent. Direct
  callers of kernel profile parsing/wire decoding must bind an explicit
  `profiled::declarations` scope. Per-runner interning/reclamation would require
  context in style values, wire decoding and host lookup, and remains deferred.
  Apple's independent ICC cache is bounded and live values own their handles.
- **CSS missing components:** `none` still becomes zero before interpolation,
  including alpha and same-space endpoints. Valid CSS is accepted. The
  narrower same-space fix was examined in round 2: a mask on `Wide` would
  cover modern gradient interiors but not legacy `rgb`/`hsl`/`hwb` (already
  reduced to `Rgba8`), gradient stop endpoints (emitted directly), or motion
  (`Value` stores premultiplied components without missingness). Completing
  carry-forward before premultiplication also needs those value paths plus
  canonical/wire and Rust/JS recorder round-tripping. That coordinated change
  remains deferred; do not rely on `none` to borrow a component. See
  [CSS Color 4, interpolating with missing components](https://www.w3.org/TR/css-color-4/#interpolation-missing).
- **Motion storage range:** `Moving` stays the compact signed 1/2048 encoding.
  Authored endpoints outside −16…15.999 linear sRGB are discrete on native
  transitions, rather than plateauing at the ceiling and jumping on completion.
  This is an intentional limit relative to browser interpolation; no wire or
  schema change was made. Spring overshoot within otherwise supported
  transitions still uses that compact frame range.
- **Inset profile colors:** predefined wide colors shade in linear sRGB with
  extended components retained; arbitrary ICC colors remain unshaded because
  the kernel cannot transform profiles. At the wide interning cap, an inset
  shade retains the authored wide color instead of clipping it.

- **Images:**
  - A tinted picture is decoded as its source's class, not as standard.
  - Animated WebP with an ICC profile composes its frames in sRGB, not in
    its own space.
  - `Picked` has no `colorSpace` or `hdr` field.
- **Colors:** `color-mix()`, relative color, and `light-dark()` with a
  profiled half.
- **Linux:** the agent's image facts and stderr do not report a tagged
  picture drawn as sRGB.
- **Canvas:** `ImageData` in half floats (HTML's `pixelFormat`), and colors
  above SDR white in a `float16` canvas (no browser ships HTML's HDR
  canvas). `sample` on the web and Linux.
- **GPU:** no gallery game opts into the HDR surface.
- **Verification:**
  - the vendored WPT cases (`kernel/tests/fixtures/css-color/`)
  - the four color Contract fixtures
  - the clips `V1`–`V4` and `anim-p3.webp`
  - V5's Chrome numbers and V8's `*.web.png` references
  - the V6 device run and the V7 measurements
  - the gallery has no video and no seam layout
