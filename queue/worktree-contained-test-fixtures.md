Worktree-contained test fixtures (2026-09-14, router chunk (c)): `exact-apple --test inherited` / `apple_artifacts_own_paths_locks_identity_and_failed_placement` creates standalone Cargo packages without their own workspace boundary. With `TMPDIR` inside this worktree, Cargo captures them into the repository workspace and refuses metadata. Three attempts stopped; make those generated packages explicitly standalone. Evidence: `target/router-test.log`, `target/router-test-retry2.log` in `exact2-wt-router`.

*Filed under “Later”.*
