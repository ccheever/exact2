//! RealWorld's public documents, rendered from the same plan and TypeScript
//! bytecode as the browser app. The native render host supplies anonymous
//! network access; it never receives a reader's kept token.

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
mod metadata {
    include!(concat!(env!("OUT_DIR"), "/module.rs"));
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    exact_logic::configured!(Data, exact_js::Module, || exact_logic::Swappable::off(
        exact_js::Module::new(
            include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc")).to_vec(),
            metadata::APP,
            metadata::GRANTS,
        )
    ));
    exact_render::main::<Data>(include_bytes!(concat!(env!("OUT_DIR"), "/app.plan")))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
