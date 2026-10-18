# Theme fixture

LLP 1121 stage 2's fixture: a design (`app.tamber`) with one app axis,
`dark-style = black | dim  under scheme=dark`, and the tokens a page reads (a
ground, a card, text, a hairline, a gradient, a shadow, an SVG's paint).
The page follows the window's modes (`setMode`, the agent's `prefer mode`);
three previews pin Light, Dim and Black beside it.

    bun scripts/agent.mjs macos --app theme-fixture "prefer color-scheme dark" "prefer mode dark-style=dim" "screenshot dim.png window"
    bun scripts/agent.mjs ios --sim <udid> --app theme-fixture "prefer color-scheme dark" "prefer mode dark-style=dim" "screenshot dim.png window"
    EXACT_APP_DIR=$PWD/apps/theme-fixture EXACT_WEB_DIST=/tmp/tf-web bun host/web/build.mjs theme-fixture
    EXACT_WEB_DIST=/tmp/tf-web bun scripts/agent.mjs web --app theme-fixture "prefer prefers-color-scheme dark" "prefer mode dark-style=dim" "screenshot dim.png"

On the web's JS target each token that varies by the axis is a custom
property, declared per tuple by `[data-x-modes]` rules (LLP 1121 D7).

It also builds for the wasm target (`web/`, LLP 1122 T2b), so the wasm dev
loop serves it with its design (`bun host/web/dev.mjs --app theme-fixture
--wasm`): the build channel, pins, scrubs and inspect run there in tests.
