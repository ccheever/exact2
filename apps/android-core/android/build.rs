//! Reuse the shared bake, then select this authored Android carrier.

#[path = "../build.rs"]
mod bake;

fn main() {
    bake::main();
    println!("cargo:rerun-if-changed=build.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let bytes = std::fs::read(out.join("app.plan")).unwrap();
    let baked = exact_plan::Plan::decode(&bytes).unwrap();
    // This app explicitly admits only its fully baked core plan, while
    // retaining the original provider, construction, binding, and Runner<Core>.
    // General plans still keep the original full owner.
    let core = exact_android::baked_core_eligible(
        &baked,
        &android_core_data::Core,
        exact_android::DataContract::CorePlan,
    );
    let (data, owner) = if core {
        (
            "android_core_data::Core",
            "exact_android::CoreOnly<android_core_data::Core>",
        )
    } else {
        (
            "android_core_data::Core",
            "exact_android::General<android_core_data::Core>",
        )
    };
    std::fs::write(
        out.join("carrier.rs"),
        format!("exact_android::host!({data}, PLAN, COMPAT, {owner});\n"),
    )
    .unwrap();
}
