//! Real VM evidence: the Windows archive executes bytecode but cannot compile source.
#![cfg(all(windows, exact_js_engine))]

#[test]
fn linked_lean_archive_refuses_source_text_after_loading_its_bytecode_prelude() {
    let error = exact_js::Module::inspect(b"globalThis.exact = {abi: 1};".to_vec()).unwrap_err();
    assert!(error.contains("the module did not load"), "{error}");
    assert!(
        error.contains("Lean VM does not support bytecode generation"),
        "{error}"
    );
}
