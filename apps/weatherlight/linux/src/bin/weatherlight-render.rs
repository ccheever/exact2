//! Weatherlight's pages as documents (LLP 1048.000 D9):
//! `weatherlight-render (--build | <location>… | --serve <dist>)`
//! ([`exact_render::main`]). Each render runs a fresh module, baked from the
//! same `app.ts` as the browser's, in an anonymous environment: only the
//! forecast's `net.fetch` grants.

// The bake's identity and placement; the rest of what it records goes unused.
#[allow(dead_code)]
mod baked {
    include!(concat!(env!("OUT_DIR"), "/module.rs"));
}
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

fn main() -> std::process::ExitCode {
    exact_render::main(PLAN, || {
        exact_js::Module::new(BYTECODE.to_vec(), baked::APP, baked::GRANTS)
            .placed(baked::TYPESCRIPT_PLACEMENT)
    })
}
