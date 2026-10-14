# Theme fixture

LLP 1121 stage 2's fixture: a design (`app.tamber`) with one app axis,
`dark-style = black | dim  under scheme=dark`, and the tokens a page reads (a
ground, a card, text, a hairline, a gradient, a shadow, an SVG's paint).
The page follows the window's modes (`setMode`, the agent's `prefer mode`);
three previews pin Light, Dim and Black beside it.

    bun scripts/agent.mjs macos --app theme-fixture "prefer color-scheme dark" "prefer mode dark-style:dim" "screenshot dim.png window"
    bun scripts/agent.mjs ios --sim <udid> --app theme-fixture "prefer color-scheme dark" "prefer mode dark-style:dim" "screenshot dim.png window"
