LLP 1026 (dynamic delivery: the app over the wire from a cloud that builds it) is
Draft r2 (2026-09-02), Charlie's exploration, no implementer. r2 is "both worlds": one
bake emits the native archive the binary embeds and a signed static update bundle;
a client keeps an on-disk store and selects at launch; native by digest identity, so
the interpreter runs only in the update window (D10); wgpu moves to the host so the
app's surfaces travel as wasm against WebGPU imports (D6); native modules stay in the
binary (D7). Level A is plan + assets at zero binary cost; Level B adds the app module
and +1.0 MB of wasmi. Measured: 72 KB module, ~1 ms to instantiate, byte-identical
answers, 8–10× on microsecond calls. Waits on §8's trades (the update economy comes
off DEFERRED; the take is the file poll + two dev-only loaders, and Stage 3's DNS-SD)
and §10's eight questions, the binary size first. 1025's link left `current/` for it.

*Filed under “Later”.*
