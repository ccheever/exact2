# shortcut-fixture

The shortcut rule on every host (LLP 1132 D11): with the world's canvas
focused, Digit1 presses the HUD's `button action="buy" aria-keyshortcuts="1"`
once, the world's own Digit1 binding (`one`) never fires, and nothing is left
held. `bun game/games/shortcut-fixture/proof.mjs <linux|web> --paranoid` runs it
continuous, under Save and under FreshGame; `bun game/games/shortcut-fixture/proof.mjs macos`
on the Mac. Pins live in [pins.json](pins.json); `bun game/prove.mjs shortcut-fixture --repin`
replaces them.
