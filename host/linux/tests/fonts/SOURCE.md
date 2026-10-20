# Color bitmap fixture

`NotoColorEmoji-subset.ttf` contains space, 😀 (U+1F600), 🌈 (U+1F308),
and 🦀 (U+1F980) from [Noto Color Emoji v2.047](https://github.com/googlefonts/noto-emoji/tree/v2.047).
Its CBDT/CBLC bitmap strikes exercise Swash's color bitmap placement.
Copyright 2022 Google Inc. The font is licensed under the SIL Open Font License 1.1; see
`NotoColorEmoji-LICENSE`.

Source: [NotoColorEmoji.ttf](https://raw.githubusercontent.com/googlefonts/noto-emoji/v2.047/fonts/NotoColorEmoji.ttf),
SHA-256 `39ee3c587e10e89669b9ff32703261d10d5f9c4dd5ad147b6b5a1c5200591817`.
Reproduce with fonttools 4.67.0:

```sh
pyftsubset NotoColorEmoji.ttf --unicodes=U+0020,U+1F600,U+1F308,U+1F980 --output-file=NotoColorEmoji-subset.ttf
```

The tests use the DejaVu faces in `scripts/fixtures/fonts/assets` for Latin text;
their source and license are recorded beside them.

# CJK outline fixture

`NotoSansCJKsc-subset.otf` contains exactly 中 (U+4E2D) and 日 (U+65E5),
the CJK characters in the mixed-script ink test. It is a subset of
[Noto Sans CJK SC 2.004](https://github.com/notofonts/noto-cjk/tree/Sans2.004).
Copyright 2014–2021 Adobe (http://www.adobe.com/), with Reserved Font Name
'Source'. The font is licensed under the SIL Open Font
License 1.1; see `NotoSansCJK-LICENSE`.

Source: [NotoSansCJKsc-Regular.otf](https://raw.githubusercontent.com/notofonts/noto-cjk/Sans2.004/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf),
SHA-256 `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`.
Reproduce with fonttools 4.67.0:

```sh
pyftsubset NotoSansCJKsc-Regular.otf --unicodes=U+4E2D,U+65E5 --output-file=NotoSansCJKsc-subset.otf
```

# Variable weight fixture

The declared-weight tests reuse `host/apple/tests/fonts/InterVariable.ttf`
in place, with its `Inter-LICENSE` beside it. Its `wght` axis spans 100–900
and defaults to 400.

# Variable width fixture

`RobotoFlex-subset.ttf` holds the glyphs of "Hamburgefonts EXACT" from Roboto
Flex, with every axis kept: `wght` 100–1000 and `wdth` 25–151 among them. The
declared-range tests draw it on both. Copyright 2017 The Roboto Flex Project
Authors (https://github.com/TypeNetwork/Roboto-Flex). The font is licensed
under the SIL Open Font License 1.1; see `RobotoFlex-LICENSE`.

Source: the copy at [SchroederNathan/react-native-motion@984e7c3](https://github.com/SchroederNathan/react-native-motion/blob/984e7c38884b95dae72e9c27259b7ef7d8287de8/apps/expo/components/animations/variable-font/assets/RobotoFlex.ttf),
SHA-256 `9b523f7d82593df0107173849ebb8c817471a1df4b4fb2c3cbf40cfd810c8281`.
Reproduce with fonttools 4.67.0:

```sh
pyftsubset RobotoFlex.ttf --text="Hamburgefonts EXACT" --layout-features='*' --output-file=RobotoFlex-subset.ttf
```
