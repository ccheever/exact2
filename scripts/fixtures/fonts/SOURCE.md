# The pinned font

DejaVu Sans 2.37, Book and Bold, as Debian ships it (`fonts-dejavu-core
2.37-8`, Ubuntu 24.04 — the fleet's builders), subset to the Latin blocks the
app's text can use. `scripts/agent.mjs` launches the Linux host with
`EXACT_FONTS` at this directory and `EXACT_FONT="DejaVu Sans"`, so text is
shaped and painted from these bytes on every machine and a pixel fixture
recorded on one matches on another (LLP 1015 §5, §8). The environment wins
over the pin; the app on a real box uses the fonts it finds.

Sources (`/usr/share/fonts/truetype/dejavu/` on `expo-build-1000`):

- `DejaVuSans.ttf` 759,720 bytes, sha256 `ae7b7855e115a5966d8b1b3f80f254ccc117ec86f9965e202ee2940453837280`
- `DejaVuSans-Bold.ttf` 708,920 bytes, sha256 `5c1247acef7f2b8522a31742c76d6adcb5569bacc0be7ceaa4dc39dd252ce895`

The subset, fonttools 4.60.2, hinting and every layout feature kept, the
family name kept (it is what `EXACT_FONT` names):

    python3 -m fontTools.subset DejaVuSans.ttf \
      --unicodes="U+0000-024F,U+02C6-02DC,U+2000-206F,U+20A0-20CF,U+2122,U+2190-21FF,U+2212-2215,U+25A0-25FF,U+2600-26FF,U+FB01-FB02" \
      --layout-features='*' --name-IDs='*' --notdef-outline --glyph-names \
      --output-file=DejaVuSans.ttf

(and the same for `DejaVuSans-Bold.ttf`). A glyph outside these blocks falls
back to the machine's own fonts, which is where a picture would stop matching:
widen the set and re-record if the app ever paints one.

`LICENSE` is the package's copyright file: the Bitstream Vera license (the
fonts may be modified and redistributed under a name without "Bitstream" or
"Vera"; DejaVu is such a name) with the DejaVu changes in the public domain.
