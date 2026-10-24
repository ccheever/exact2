//! Excord: a Discord client (`apps/excord/llp/0001`).

include!(concat!(env!("OUT_DIR"), "/module.rs"));
include!(concat!(env!("OUT_DIR"), "/baked.rs"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    let mut module = exact_js::Module::new(APP_BYTECODE, APP, GRANTS);
    // A large account's READY is megabytes of JSON, and parsed it is several times that: a 16 MB READY
    // ran out of the default 64 MiB heap.
    module.set_max_heap(256 << 20);
    module.placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_apple::host!(AppData, exact_plan(), exact_compat(), None, std::ptr::null(), app_data; linked = EXACT_LINKED);
