//! Bake `app.contract` through `app.ts` into `OUT_DIR` at build time: the plan,
//! the module's Hermes bytecode, its identity and the compatibility id, for
//! `src/lib.rs` and `exact_linux::canvas_jni!` to embed. The bake is the Linux
//! host's (`apps/messages/linux/build.rs`): the Canvas host is that host's
//! presenter under Android's own Canvas, and has no update store either.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // The library is empty off Android (src/lib.rs): nothing to bake.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    exact_js_bake::build(std::path::Path::new(".."), "linux").expect("bake Shelf");
}
