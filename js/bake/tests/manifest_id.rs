//! An `app.ts` that names no `appId` is baked with `app.json`'s (LLP 1030 D2), and its receipt carries it.
use exact_js::Paired;
use exact_js_bake::{bake, Tools};

#[test]
fn a_module_without_an_app_id_takes_the_manifests() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let dir = std::env::temp_dir().join(format!("exact-manifest-id-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).unwrap();
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    write(
        "app.json",
        r#"{"app": {"id": "test.exact.manifest", "name": "Manifest"}}"#,
    );
    write(
        "app.contract",
        "component App\n  resource message = message() as shape string\n  view\n    text message\n",
    );
    write("app.ts", "import type { Sources, Answer } from './app.contract.d.ts';\nexport const grants = '';\nconst sources: Sources = { message: () => 'hi' };\nexport const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);\n");
    let baked = bake(&dir, &Tools::default()).unwrap();
    let receipt: serde_json::Value = serde_json::from_str(&baked.receipt).unwrap();
    assert_eq!(receipt["appId"], "test.exact.manifest");
    Paired::decode(
        &baked.receipt,
        &baked.plan,
        baked.bytecode.clone(),
        "test.exact.manifest",
        "",
    )
    .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
