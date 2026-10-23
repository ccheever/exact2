# Tamber for Figma (private plugin; LLP 1121 §5 stage 10b)

This plugin keeps a Figma file's variables in step with an app's design (`app.tamber`). It is a client of the
design server: it reads, and it proposes. It never writes the design.

## Run it

1. **Start the server for the plugin.** Run the app's dev loop, or `bun exact.mjs tamber serve <app>`, with
   `--allow-plugin-origin`. A plugin's UI is a sandboxed page (`Origin: null`), and that flag lets it call the wire
   with a paired token. Figma on another machine reaches the server through a tunnel that has a certificate of its
   own: `--trust-tunnel <port>` with `--allow-host <its public name>`. Figma can't pin the TLS door's self-signed
   certificate.
2. **Build the plugin.** Run `bun tools/figma-tamber/build.mjs`. Then in Figma, choose Plugins › Development ›
   Import plugin from manifest… and pick `tools/figma-tamber/manifest.json`.
3. **Pair.** On the server's machine, run `bun exact.mjs tamber pair <server url> --scope propose`. In the plugin,
   enter the server URL and the code. The server and its token persist together in Figma's `clientStorage` (this
   user, this machine) until the next pairing, never in the file. A code or a token goes only to an https address or
   to this machine over http (`http://127.0.0.1:<port>`); the plugin follows no redirect, so the token goes only to
   that server. It speaks `tamber/1.14` or later, and checks the server's `hello` before every pull and push.

## Pull and push

- **Pull** reads `export dtcg` and writes the variables:
  - The **Tamber** collection gets one mode per full tuple of the design (its app axes, light and dark), except that
    light tuples differing only by an axis `under scheme=dark` share one light mode, since the design has one light
    side for them (the theme fixture: `light`, `dark-style=black, dark`, `dark-style=dim, dark`). A push writes that
    mode's value to each of them.
  - If the file's plan allows fewer modes than that, you get one collection per app tuple instead, each with Light
    and Dark modes.
  - Colours become COLOR. Lengths, numbers and durations become FLOAT, in px and ms. Families become STRING.
  - Types, shadows and easings have no Figma variable type. They're skipped, and the plugin says so.
  - The export's default context is each new collection's default mode.
  - Families with a comma in a name are quoted, as CSS quotes them (`"ACME, Sans", serif`).
  - The export is kept in the file as the pull's baseline, in parts of under 100 KB.
  - If a value changed in both Figma and the design since the last pull, by token and context, the plugin asks
    which one to keep. A Figma edit whose token or context the design dropped counts too.
  - If the design's contexts changed, the plugin asks before rewriting. A re-pull removes the modes and the
    variables of tokens the design no longer has, and a collection the new layout doesn't use. Anything in the file
    bound to them loses it.
  - The plugin marks each variable with its token, and keeps each collection's id and each context's mode id, so a
    variable, mode or collection renamed in Figma is still the plugin's. A mode's name is the context, cut to Figma's
    40 characters. A default mode whose context the design dropped stays, named `unused: …`, since Figma can't remove
    a default mode, and no surviving context loses its mode.
  - Before a pull, the plugin lists the values it can't read (an alias to nowhere, a mode or collection deleted),
    since a pull would write over them unseen. A collection named `Tamber` that the plugin didn't
    make is refused, not taken over.
  - A pull is one undo step. If Figma refuses any part, the plugin undoes all of it: the earlier collections, modes,
    variables, values and kept export come back as they were.
  - An alias is followed to its value, into a library's collection too. One that goes nowhere, or round in a cycle,
    is listed on push and sent as it was pulled. A pull lists every alias in a Tamber variable first, then writes the
    design's value over it.
- **Push** writes Figma's values back into that export and sends it as `import`, with the pulled export as its
  `baseline`. The server holds the result as a proposal, planned against the design as it is now. Accept it in the
  design tool.
  - A changed value whose line is one dial alone is proposed as that dial's change, listing every token it moves.
  - Any other changed value is a literal, marked with what it no longer follows.
  - The plugin lists every value the design couldn't take, and why. For example, a length changed in Light only:
    both schemes take one value.

## Paper and Pencil (no plugin)

Paper and Pencil each run a local MCP server while the app is open. An agent that holds the local token bridges
them; it's a client like any other, so every check holds:

- **Into the tool:** `bun exact.mjs tamber op <url> '{"method":"export","kind":"dtcg"}'` (or `resolve`), then set
  the tool's variables through its MCP server (Paper: CSS variables; Pencil: its variables and themes).
- **Back:** read the tool's variables, write them into that export, and send `import` with the export as
  `baseline`. A person accepts the proposal in the design tool.

Pencil's `.pen` files, kept in the repo, also import directly (*tamber/1.17*): `bun exact.mjs tamber import pen design.pen <url>`, or `import {kind: "pen"}` from a client. Their themes become axes and their variables tokens (`docs/tamber-protocol.md`, `import`).

## Tests

`bun test tools/figma-tamber/src/` covers the mapping, and covers the main thread against an in-memory stand-in for
the Figma Variables API. The real-Figma run is the stage 10b gate's evidence.
