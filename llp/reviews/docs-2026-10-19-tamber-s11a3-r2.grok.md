# Docs review: LLP 1121 stage 11 A3, package designs as built, round 2 of 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at the commit under review.
- **Method:** one brief (sha256 `800e3625bf4b7046596b79cc0ca8a30ed13d0e2203bcdcef2ae31b68eafa9db5`), shared by both reviewers, blind to the other review; commit 141add0a04 (reviewed: 8d65da90a6). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (4 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in 141add0a04: the one-name rule as the app's over a package's, the nine base names and that a package may not declare them, where refusals point, the guide's file locations and component block and ids, the collision as an app's two packages, and that the tamber subcommands read the app's design alone (check with contract build).

No BLOCKING findings. Four SHOULD-FIX items, one NIT.

**SHOULD-FIX** — State the nine base names, and that a package may not declare them — `llp/1121-tamber.rfc.md:855`

A package token may name only the nine strings in `BASE` (`space.xs`, `space.sm`, `space.md`, `space.lg`, `space.xl`, `radius.sm`, `radius.md`, `radius.lg`, `radius.full`). `space.*` and `radius.*` also cover names the fold refuses, such as `space.2xl` and `radius.xs` (`tamber-unknown-token`). The same paragraph's "`<name>.…`" rule (`:846`) does not say those nine are refused even when they match the namespace: a package named `space` cannot declare `space.md` (`tamber-package-name`, `contract/lower/src/theme/packages.rs:164`). The guide's list at `docs/contract-for-agents.md:3550` matches the nine, and it still does not say declaring one is refused.

**SHOULD-FIX** — Don't say every refusal is on a token line — `llp/1121-tamber.rfc.md:857`

A fold refusal of a token (bad name, duplicate, unknown axis, unknown reference) is that token's line in the package file, with banded `line N` text unbanded. A namespace collision and a missing or outside `"tamber"` path are `tamber-package-name` and `tamber-unreadable` on the file at line 0 (`contract/cli/src/sources/packages.rs:132` and `:158`, `Span::default()`). A forbidden block points at that declaration's line, not at a token. The guide's "at its line" (`docs/contract-for-agents.md:3553`) has the same hole for the collision it just stated.

**SHOULD-FIX** — Give the guide the real file locations — `docs/contract-for-agents.md:3542`

D11 matches the driver: `<name>.tamber` beside what `exports` maps `.` to, otherwise beside `index.contract` (`contract/cli/src/resolve.rs:414`). The guide stops at `exports`. It also does not say a `"tamber"` path must stay inside the package: missing, or leading out, is `tamber-unreadable`, while a missing default `<name>.tamber` is simply no design.

**SHOULD-FIX** — The shared last segment is a link refusal, and `component` is refused too — `docs/contract-for-agents.md:3548`

`@acme/reading` and `@other/reading` may each ship a design. The build refuses only when one app links both and both have a design, and the error names both (`:3552`). D11 says that (`llp/1121-tamber.rfc.md:846`). "Can't both ship" reads as a publishing ban. Separately, "no … params" does not name a `component` block. That block is `tamber-package-block`, same as `axes`, `dials`, `params`, `icons`, and `exports`; components are stage 12. D11 lists it (`:851`).

**NIT** — Cite the diagnostic ids in the guide paragraph — `docs/contract-for-agents.md:3554`

The surrounding guide names the id in the sentence (`tamber-contrast`, `tamber-param-scope`). This paragraph never names `tamber-package-name`, `tamber-package-block`, `tamber-unknown-token`, `tamber-unknown-axis`, or `tamber-unreadable`.

What checks out:

- File discovery in D11: entry from `exports` of `.`, else `index.contract`; the `"tamber"` field; inside the package; transitive `use`s, including a vendored package reached by a relative `use`.
- Namespace from the package's own `package.json` name, not the name a `use` installed it under; header `tamber <name>`; tokens `<name>.…`.
- Collision only when both linked packages ship a design; the error names both; no app line can tell the tokens apart.
- Identity: the token keeps its name; `DesignSource.home` is `<package>/<file>` (`@exact/reading/reading.tamber`); the `#<token>` form is marked stage 11 A2, and the server does not do that today.
- Theme only. The app's definition replaces the package token whole, `when` lines included. `when` lines name the app's axes.
- References, judged from the fold: own declared names (an app token added under the namespace does not count; one the app overrides still does), roles, `pair=`, and no dial in `Checked.reads` (not a dial's reach; `rgb(…)` and `#00face` are not reads). Cross-package references are queued. `tamber gen` reads the app file alone; `contract types` lists the package tokens the plan keeps.
- The guide stays in the same concrete style as the section around it.
