# FOLD environmental comparison — 2026-09-20

Pristine detached worktree: `origin/lane/game` = `64b5af1`, under
`~/lanes/gamenext/scratch/fold/engine`. Its tracked status stayed empty after
verification. No merge fixes were applied there. Environment: Linux x86-64,
Bun **1.3.12** (the package.json pin), inherited debug=0/incremental=0,
`EXACT_UPDATE_TRUST=development`. Disk: 97 GiB before its cold builds; 94 GiB
at completion. Its 3.3 GiB Cargo target, incidental 40 MiB root target and web
dist were deleted immediately after the comparisons. Logs and commands remain
in `~/lanes/gamenext/scratch/fold/{logs,pristine-results.json}`.

The exact prior diary names were passed as libtest filters with `--exact` and
`--nocapture`: game workspace (15 names), each of the three app workspaces (one
name each), exact-gpu (three names). Generated shells kept `--locked --offline`.
The initial app preparation found uncached `cc 1.4.7`; `cargo fetch --locked`
populated the cache, then the same unchanged locks and tests ran. The root and
game workspaces did not acquire generated-shell resolution flags.

All 20 names present in pristine engine fail identically on both trees: 17 at
adapter creation and three at native registry load. No pristine named test
passes while its merged counterpart fails. The 21st, land's strict
Fox residency test, does not exist in pristine: zero matches is **not a pass** or
a same-test environmental proof. On the merge its first real adapter request
fails; none of its residency assertions executes on this machine.

| Exact test | Pristine engine | Folded tree |
|---|---|---|
| `models::retirement_regressions::pending_names_count_retired_bytes_and_compaction_keeps_hero_handles` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `placed::tests::captured_children_share_draw_and_hit_depth_and_a_wall_occludes_them` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `quad_tests::equal_depth_uses_layer_then_slot_and_mask_respects_cutoff_with_signed_scale` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `quad_tests::particle_storage_and_pipelines_prepare_only_with_emitters` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `quad_tests::invalid_quads_are_journaled_without_refusing_valid_neighbors` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `quad_tests::retired_sprite_waits_for_redelivery_and_reuses_identical_texture` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `quad_tests::same_owner_sprite_then_particle_is_pinned_and_adjacent_sprites_batch` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `renderer::packing_tests::oversized_retired_arenas_pack_without_reuploading_live_meshes` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `skinning::normal_tests::skinned_normal_is_inverse_transpose_under_scaled_rotated_joints` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `skinning::tests::displayed_affine_matches_gpu_with_animated_translation_scale_and_rotated_child` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `surface::residency_tests::identical_redelivery_survives_entry_and_post_acceptance_budget_compaction` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `surface::residency_tests::restoring_fox_uploads_zero_asset_bytes_after_ready` | Absent (0 matches) | Fails: no GPU adapter; residency unverified |
| `surface::tests::placement_and_renderer_share_warnings_across_restore_and_prune_dead_followers` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `surface::tests::world_surface_retires_and_readds_a_real_placed_child` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `affine_attachment_pixels_match_transformed_vertices_and_detach_cleanly` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `first_presented_fox_matches_current_pose_in_fox_rectangle` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `rendered_atlas_and_mid_fall_restore` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `primitive_particles_render_and_do_not_pick` | Fails: No suitable graphics adapter found | Fails: No suitable graphics adapter found |
| `native::placement_abi_tests::retiring_each_releases_textures_and_zero_frame_releases_a_capture` | Fails: native registry load 1, expected 0 (no device) | Fails: native registry load 1, expected 0 (no device) |
| `native::placement_abi_tests::replacement_lost_during_preparation_refuses_then_retries` | Fails: native registry load 1, expected 0 (no device) | Fails: native registry load 1, expected 0 (no device) |
| `native::placement_abi_tests::recovery_of_a_healthy_device_does_not_prepare_again` | Fails: native registry load 1, expected 0 (no device) | Fails: native registry load 1, expected 0 (no device) |

The four Apple test names ran from `placement-apple.test.mjs`, unchanged. Caltrain
first skipped because no complete web dist existed. A real pristine Caltrain web
bake succeeded (53.51 s), then the exact test ran and failed on missing Chrome.
That establishes the current missing-browser limitation; it **does not reproduce
or explain** the previous lane's 60-second Chromium timeout. That historical
cause remains unresolved until the receiving Chrome host executes it.

| Exact web test | Pristine engine | Folded tree |
|---|---|---|
| `Mac clears zero-sized and display-none captures` | Missing xcrun; process status undefined | Missing xcrun; process status undefined |
| `IOS clears zero-sized and display-none captures` | Missing xcrun; process status undefined | Missing xcrun; process status undefined |
| `Mac hidden placement box is zero, not the kernel frame` | Missing xcrun; process status undefined | Missing xcrun; process status undefined |
| `IOS hidden placement box is zero, not the kernel frame` | Missing xcrun; process status undefined | Missing xcrun; process status undefined |
| `Caltrain web frame-only stack remains a plain column` | Chrome ENOENT after successful bake | Chrome ENOENT after successful bake (13.85 s) |

The three incoming ordinary GPU failures also ran by exact name on pristine.
Their results match the merge. The should-panic case reaches adapter refusal
instead of its expected particle-preparation panic on both trees.

| Additional exact test | Pristine engine | Folded tree |
|---|---|---|
| `quad_tests::r14_unprepared_particle_draw_names_the_refusal` | Adapter refusal; wrong panic for should-panic expectation | Same |
| `quads::retained_tests::r14_native_children_reserve_order_before_frame` | No suitable graphics adapter found | Same |
| `renderer::e10_tests::full_glow_blooms_without_clipping_the_lit_pixel_to_white` | No suitable graphics adapter found | Same |
| `reused Chrome clears IndexedDB, history and held keys/contacts between stages` | Chrome spawn ENOENT | Same |
| `E10 browser buttons retain UA keyboard focus and hover feedback` | Chrome ENOENT, then null Cdp output stream | Same |
| `agent_snapshots_and_pick` (Greybox) | Fails: expected tree omits incoming Glow | Passes: actual tree snapshot includes Glow; original assertions retained |

Game Bun's generated-game test additionally built the untouched starter, ran its
Linux assertions with no failures, refused empty pins and successfully built web;
its browser stage then failed Chrome ENOENT. That complete test was not rerun
on pristine; no same-test comparison is claimed for it.

The extra pristine commands, unmodified tracked-status checks and logs are in
`final-results.json` / `logs/final-pristine-*`. The second cold comparison started
with 88 GiB free. Its Cargo outputs were deleted immediately after the run.
Adapter-optional tests returning success certify no GPU execution.

A final pristine tooling comparison runs `bun test ./scripts/app.test.mjs`:
**29 pass / 2 skipped**, exactly matching the repaired merge. Before repair,
20 isolated shell cases could not import the newly required scene module, and
`R14 ordinary no-lock workspace reaches buildBake` failed on an unconditional
`app.prepare()` absent from the pristine implementation. These were merge defects,
fixed in one round; they are not classified as environmental. Original assertions
remain. The two new standalone Lanterns capture failures likewise required the
missing `Follow` registration; their existing assertions now pass. Logs:
`logs/repair-{app-tooling,pristine-app-tooling,apps}.log`.

After the last pristine comparison its tracked status was empty, its build outputs
were removed, and the detached scratch worktree was removed. No pristine passing
test remains failing on the merge. The land-only strict Fox acceptance and the
historical Chrome timeout remain explicitly unproven, as above.
