**Published dependency validation** (2026-09-19): the isolated Markdown
candidate restores origin's `ureq` 3.4.0 lock entry for clean published Ibex
`9cbf9e62`. Final gates must use that adjacent clean checkout and explicitly
identified Hermes/compiler artifacts; earlier runs used a dirty sibling
requiring 3.4.2. The unrelated sibling changes remain untouched and are not
authorized for publication by this Exact task. A different Ibex checkout on
the M5 requires 3.4.2, so the path dependency still makes `--locked` depend
on which sibling checkout resolved the lockfile last.

*Filed under “Next, in order (2026-08-29)”.*
