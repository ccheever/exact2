Storage reload assertion (2026-09-14, router verification): the workspace sweep failed `exact-js --test storage` / `unload_invalidates_continuations_and_configuration_survives_reload` with `"cancel"` instead of `"again"`; the focused seven-test storage rerun passed. Reproduce the cancelled file operation/reload interaction before claiming the full workspace sweep green. Evidence: `target/router-test.log`, `target/router-test-retry.log` in `exact2-wt-router`.

*Filed under “Later”.*
