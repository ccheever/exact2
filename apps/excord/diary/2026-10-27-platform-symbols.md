surfaces: windows, android
agent: Claude Code (Claude Opus 5.5)
task: the channel header's Back and More in Windows' and Android's own shapes

### Rough

- The channel header's `symbol:back` drew Apple's chevron and `symbol:more` an ellipsis in a
  circle on Windows, where WinUI's Back is an arrow and its More three bare dots. The painted
  presenter (Windows, Android, desktop Linux) drew every role from the table's one path.
  Fixed in exact2: the schema's `symbolGlyphs` gives Windows its arrow and dots and Android
  Material's `arrow_back`, `more_vert` and `share`; the web and desktop Linux keep the
  table's shapes.
- Neither Windows nor Android runs on this Mac: the shapes are held by a kernel test and a
  presenter test that paints each set and reads its pixels, not seen on a device.
- A literal `symbol:sf/chevron.backward` is lowered to the role `back`, so it takes the
  platform's arrow too; only an icon token keeps its own drawing.

### Needed

- A directional symbol mirrors right to left on no painted host yet (LLP 1115.001 D14).

### Checkpoints

- Windows and Android: Back is an arrow and More dots, by test only.
