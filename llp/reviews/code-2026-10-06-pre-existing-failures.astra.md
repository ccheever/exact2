# Round 1 (the seven test and kernel commits)
Two defects in the kernel change:

- **[P2] The node limit is applied across both branches** — [kernel/src/region/state.rs:680](/Users/admin/projects/exact2-wt-pre/kernel/src/region/state.rs:680). `REGION_NODES` limits each branch separately, but `holds_absolute` counts their combined descendants. A static owner with 2,048 content nodes and 2,049 pending nodes, all static, is incorrectly refused. Growing an already registered region across that combined threshold also invalidates it and blocks subsequent layout. Count each branch separately.

- **[P2] The visit limit does not bound traversal allocation** — [kernel/src/region/state.rs:683](/Users/admin/projects/exact2-wt-pre/kernel/src/region/state.rs:683). `stack.extend` copies every child before the next limit check. A branch with 100,000 immediate children therefore allocates and copies all 100,000 entries despite the 4,096-node limit. Revalidation repeats this on subsequent commits while the oversized region remains registered. Check the pending traversal size before extending.

Otherwise, I found no T1 positioning defect: sticky lowers to relative layout, fixed is unsupported, and adding absolute descendants invalidates the candidate and prevents subsequent region layout. The six test-only changes faithfully preserve their intended checks.

Verification: 70 focused tests passed using existing binaries; one was ignored. The transport test was blocked by sandbox socket permissions. No files modified.
# Round 2 (kernel walk; the bake deferral, round 1)
No concrete defects found in either commit.

- **854c3b132:** The limit resets per branch. The pre-copy check counts visited nodes, queued nodes, and children, preventing an oversized child-list copy.
- **218bec42d:** The typed path preserves resource deferral with and without `else`; caught-then-thrown errors remain fatal. Mutations are unchanged. `parse_for` preserves the typed error, and normal baking never completes fetches. Worker placement and the web build introduce no regression. No counter/token references remain.

In-memory prelude checks passed for filesystem, SQLite, caught-then-thrown, and post-fetch refusals. Cached region tests passed, but the binary predates the latest source; no fresh build was run. No files were modified.
# Disposition

Round 1: the walk now counts each branch on its own and copies nothing past REGION_NODES (854c3b132); the arrange test cites 3f8cbb165. Round 2: no defects. Found after the reviews, at load 200+: the deadline transport test's silent socket can stay open past 45 s on URLSession's side after the cancel, so only the trickling socket's close is asserted (98e47b125); not re-reviewed.
