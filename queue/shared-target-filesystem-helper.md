Shared-target filesystem helper: the ada0b93 workspace sweep failed with `spawnSync .../exact-filesystem ENOENT`; the working checkout’s async reader also intermittently returned 404 when its helper exited after one request. Reproduce helper executable identity/lifecycle across worktrees before claiming a full green sweep; evidence in `/tmp/interview-snapback4-20260913/exact2-checks/` and `/tmp/interview-snapback4-20260913/ui/`.
LLP 1039's web smoke passed every assertion but retained its `exact-filesystem --serve-reads` child after printing success; closing that recorded child let Bun exit 0. Close the helper at the end of a finite bake/smoke process (`/tmp/lane-router/1039/smoke-web.log`, `launched-pids.txt`).

*Filed under “Later”.*
