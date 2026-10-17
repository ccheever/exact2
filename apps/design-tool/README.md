# The design tool

LLP 1122's "living Figma" over an app's Tamber design (LLP 1121), unnamed
until §9 Q1 names it. `bun exact.mjs tamber tool <app>` builds it if stale
and opens it at `http://127.0.0.1:7740/__tamber/tool/`, served by the app's
design server; with the app's dev loop running, which holds the design,
`--via <dev url>` reaches that loop's server instead. Development only: it
is never bundled into an app's release build.

The tool always lives at `127.0.0.1:7740` (its grant names that one origin),
so one runs per machine, on one project. To switch projects, stop it with
ctrl-c and run `bun exact.mjs tamber tool <other app>`, adding `--via <its
URL>` if that app's dev loop is running. A second `tamber tool` for the same
app reuses the running one; for another app it is refused, naming the app
whose design the port serves.

- The canvas (T2, LLP 1122 D3) frames the running app on its dev loop's
  port as `localhost`, at a phone, tablet or desktop size, with a link that
  opens it in a tab at the URL the dev loop prints (`hello.page`). The frame is sandboxed short of navigating the tool, and the
  tool never scripts it; only a loopback origin is framed, 127.0.0.1 as
  `localhost` on its port, which the dev loops answer on [::1] too. That is
  another site than the tool's, so a browser with site isolation (Chrome's
  desktop default) runs the page in its own process, and a busy one cannot
  stall the tool. It is another origin, too: the canvas keeps its own
  cookies and storage, apart from a tab at the printed 127.0.0.1 URL, which
  "Open in a tab" opens. A dev loop's HTML
  may be framed by the tool's origin and its own, no other. With no dev loop
  (`tamber tool` alone), it says how to start one and open the tool through
  it (`--via`, after stopping this tool).
- Builds (T2, LLP 1122 D4) lists every running copy the server knows
  (`hello.builds`, then the `builds` and `build-status` events), the frame's
  among them: its host and id, its effective scheme, contrast, mode and root
  font size from its own status (an app's own choice beats a pin), its pin,
  and whether its status has caught up with the pin's revision. A pin (one
  build or every build: scheme, contrast, each axis, root font size) is
  `pin`; one axis on one build is `preview`; a build's `unsupported` answer
  is shown by name. The tool names itself on its long poll, so its pins hold
  while it runs and are handed back about 40 s after it closes (the 35 s
  presence lease and the server's 5 s sweep); it sends no unload request.
- Scrubbing (T2, LLP 1122 D6) is chosen per edit ("Scrub live" on a dial
  or cell edit, never the default): each value the draft takes is a `scrub`
  frame previewed on every running copy and written to no file, while the
  tool says "Previewing". Keep is `scrub-release`, one write undone in one
  step (refused, it names who moved the file); Cancel or Escape is
  `scrub-cancel`; an end the server makes (another write, idle 30 s, the
  tool gone) is said. An edit that rebuilds, or a tool with no dev loop,
  offers no scrub, and says why.
- The inspector (T2b, LLP 1122 D5): Inspect turns inspect mode on every
  build that can (the wasm page now). The toggle follows the server's
  `inspect` events, so another client taking a build, or this tool's
  presence lapsing, shows at once. A click in one selects a view, and the
  server's `selection` shows here: its source and component call sites, its
  modes, and each row with where its value comes from. A row the node itself
  sets to a token offers that token's cell in the grid, on the branch that
  won for the selected copy (its modes, and the node's own `color-scheme`,
  else its build's scheme), with T1's edits and T2's scrubs; an inherited or
  derived row links its token; a style's or the tag's row says it comes with
  template editing (T5). A selection stops being current, and offers
  nothing, when its build goes, reloads or moves on (a pin, a mode, a new
  plan or design), its page leaves inspect mode, or the design's axes
  change; it stays so until a new selection. A token's impact is its
  `uses`, static source sites, and Outline shows them on every build in
  inspect mode, naming the builds told.
- `app.ts` is the `tamber/1` client: the token from `GET /__tamber/token`
  once, every request a `POST /__tamber`, and changes followed by the long
  poll, `POST /__tamber/events`. Its grants name the tool's one origin.
- Every write is `design-tool:<person>`, the person asked once and kept.
- The dials panel and the token grid are drawn from `describe` and
  `resolve`; a cell edits the branch it shows, and every edit is proposed
  (its diff and "live" or "rebuilds") before Apply writes it. Undo and redo
  are this run of the server's inverses: a new run starts them again.
- A cell's value has typed fields by its kind (`parts.ts`): a length's
  amount and unit, a number, each shadow's inset, offsets, blur, spread
  and colour (a literal or a token's name), a type's style, weight, size,
  line height and family. A part the value leaves out is offered empty. A
  field edits only its own part, and the rest of the line stays exactly as
  written, so the one line `set-token` writes changes only there. A value
  that no field fits (an expression, a `calc()`, a named length) has only
  the text field, which every value keeps. Fields keep their cut while you
  type, so a part on its way (`1` before `1px`) leaves every field where it
  is. A required part left empty drafts nothing and says so, since a later
  part would be read in its place. A value that names a token (a shadow's
  colour) keeps the name and links to it. A length, number or shadow edit
  can be scrubbed live; a type edit only where `propose` says it is live.
- Kinds that rebuild say so before the gesture. A motion token (duration,
  easing) says "Rebuilds" when it is chosen, and Apply writes it with no
  scrub. On the JS target's dev loop, `propose` answers `rebuild` for a
  token the page writes inline (a `font-weight` number, an SVG attribute,
  a type or family token), so the chip says so before a scrub is tried.
  Icons are listed read only, with their sources, since no operation
  writes one.
- Changes lists every write since the tool opened, the tool's own and
  others' (agents, hand edits), with the token values before and after.
  Commit commits what the session wrote, with a message naming each change
  and who made it; the server refuses a file that was not only the
  session's, and never pushes. History reads the design files' git log as
  token edits; Revert holds a past commit's inverse for review.
- Agents are clients like the tool (LLP 1122 D10): each change says who
  made it, in that client's colour, and the grid outlines the cells it
  changed (by the latest client to change each), fading oldest first, one
  each 3 s. Proposals lists the held proposals, each with its
  proposer, diff and items: Accept, Accept picked (some items) or Drop. A
  proposal accepted in part, or whose files moved since it was held, comes
  back recomputed on today's files, to be reviewed and accepted again.
- Import a theme (T1a, LLP 1122 D11): pick one shadcn CSS file or Tailwind v4
  sheet, or DTCG JSON (several files, a mode each) and the server's importer
  holds it as a proposal. Its review lists what it maps: each token with
  each mode's value and swatches (a reference to a token said, unpainted),
  each in until unticked, every mode's value editable before accepting; an
  axis it adds; a mode's value it drops; the role overrides it offers (each off until ticked;
  one it cannot offer says why), every value it did not map with the
  reason, and the count: source values = mapped + reported. Accepting is
  one transaction, and Undo one step. Astryx's import comes from its CLI
  (`bun exact.mjs tamber import astryx …`) and is reviewed the same way.
- `app.test.contract` runs against `stub/tamber-stub.ts`, canned replies;
  `scripts/design-tool.test.mjs` drives the built tool against a real
  server on a copy of `apps/theme-fixture`.
