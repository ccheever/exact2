# Device-free world surface

`WorldSurface<G: exact_world::Game>: exact_gpu::Surface` owns a `Sim<G>`.
`module!(Game)` registers it as `world`, with typed argument defaults. An app
selects it with `game.world: true`. Tally is the nonspatial consumer: ordinary
Contract text and action buttons, with a surface carrier but no canvas drawing.

Binding constructs and takes one fixed tick immediately. Ready means bound and
at least one tick taken. Lifecycle, keys, named button controls, scalar axis
controls, clock ownership, messages, publications and exact save/restore work
without presentation. `render`, device callbacks and child composition use
Surface defaults. `sim(&self) -> Option<&Sim<G>>` exposes read-only simulation.

Agent JSON transport lives here. `state`, `tree`, `clock` and `logs` use the
kernel's bounded inspection writer; unsupported operations return an error.
Requests admit 16 KiB, entity listings 512 rows, output 65,536 bytes/visits,
clock requests 216,000 ticks, settle 3,600 ticks and input queues 1,024 events.
Kernel admission bounds still apply to each operation. Tally has exactly 15
entities and at most twelve card visits per game operation; its heartbeat makes
settle exhaust its explicit budget. The proof's `clock +1000` publication
assertion is a negative control against a frozen or absent world.

The kernel has no caller-clock getter. The adapter checkpoint contains a version,
eight bytes of caller time and the opaque Sim save. Host time is rebased on open;
queued input remains in the Sim checkpoint. No kernel files are changed.

Measured on this Linux box: Tally construction 91 allocations / 15,459 requested
bytes; first tick 6 / 672; exact restore 159 / 33,432, for a 3,623-byte Sim save.
The test reuses the kernel counting allocator source without adding unsafe code.
Artifact and host startup measurements are recorded below when completed.
