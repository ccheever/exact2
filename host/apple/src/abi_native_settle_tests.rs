//! Exercise generation carry across a real, asynchronously loaded native image.
use super::*;
use std::{fs, process::Command, time::Instant};

#[test]
#[ignore = "async lane: compiles a native module fixture"]
fn native_image_settle_preserves_input_after_preparation() {
    let dir = std::env::temp_dir().join(format!("exact-apple-settle-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let entered = dir.join("entered");
    let release = dir.join("release");
    let library = dir.join(if cfg!(target_os = "macos") {
        "fixture.dylib"
    } else {
        "fixture.so"
    });
    let mut metadata = exact_plan::bytes::Writer::default();
    metadata.u32(exact_logic::ABI);
    metadata.u8(0);
    metadata.string("test.exact.storage.reload");
    metadata.string("");
    let mut unit = exact_plan::bytes::Writer::default();
    unit.u32(exact_logic::ABI);
    unit.u8(1);
    unit.u8(0);
    Value::Unit.encode(&mut unit);
    let array = |bytes: Vec<u8>| {
        bytes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let source = dir.join("fixture.c");
    fs::write(
        &source,
        format!(
            r#"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>
__attribute__((constructor)) static void prepare(void) {{
    FILE *f = fopen("{entered}", "w"); if (f) fclose(f);
    while (access("{release}", F_OK) != 0) usleep(1000);
}}
typedef struct {{ uint8_t output[256]; uint32_t length; }} Session;
uint32_t exact_logic_abi(void) {{ return {abi}; }}
uintptr_t exact_logic_create(void) {{ return (uintptr_t)calloc(1, sizeof(Session)); }}
void exact_logic_destroy(uintptr_t session) {{ free((void*)session); }}
uintptr_t exact_logic_alloc(uint32_t length) {{ return (uintptr_t)malloc(length); }}
void exact_logic_dealloc(uintptr_t ptr, uint32_t length) {{ free((void*)ptr); }}
uint32_t exact_logic_call(uintptr_t session, uintptr_t ptr, uint32_t length) {{
    uint8_t metadata[] = {{{metadata}}}, unit[] = {{{unit}}};
    uint8_t *bytes = ((uint8_t*)ptr)[4] == 0 ? metadata : unit;
    uint32_t count = ((uint8_t*)ptr)[4] == 0 ? sizeof(metadata) : sizeof(unit);
    memcpy(((Session*)session)->output, bytes, count);
    ((Session*)session)->length = count; return 0;
}}
uintptr_t exact_logic_output(uintptr_t session) {{ return (uintptr_t)((Session*)session)->output; }}
uint32_t exact_logic_output_len(uintptr_t session) {{ return ((Session*)session)->length; }}
"#,
            entered = entered.display(),
            release = release.display(),
            abi = exact_logic::ABI,
            metadata = array(metadata.into_vec()),
            unit = array(unit.into_vec())
        ),
    )
    .unwrap();
    let output = Command::new("cc")
        .args(["-shared", "-fPIC"])
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let image = fs::read(&library).unwrap();
    let mut plan = contract::compile("component App\n  state count = 0\n  action increment\n    count = count + 1\n  view\n    button press=increment testId=\"increment\"\n      text `${count}`\n").unwrap();
    plan.app_id = "test.exact.storage.reload".into();
    let plan = plan.encode();
    let card = |name: &str, bytes: &[u8]| {
        let hash: String = exact_data::crypto::digest(exact_data::crypto::Sha::Sha256, bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        serde_json::json!({"file":name,"bytes":bytes.len(),"sha256":hash})
    };
    let receipt = serde_json::json!({"version":1,"kind":"rust","abi":exact_logic::ABI,
        "appId":"test.exact.storage.reload","grants":"","executor":"native","target":exact_logic::TARGET,
        "plan":card("app.plan", &plan), "module":card(if cfg!(target_os="macos") {"app.module.dylib"} else {"app.module.so"}, &image)}).to_string();
    let source = || exact_logic::Swappable::native(StorageModule::default());
    let mut bridge = Bridge::new();
    bridge.boot(&plan, source(), Hooks::none(), 390.0, 844.0);
    bridge.data_ready();
    let prepare = |bridge: &mut Bridge<_>| {
        let mut payload = plan.clone();
        payload.extend_from_slice(receipt.as_bytes());
        payload.extend_from_slice(&image);
        bridge.input_write(&payload);
        bridge.prepare_module(
            [plan.len(), receipt.len(), image.len()],
            source(),
            Hooks::none(),
            390.0,
            844.0,
        )
    };
    let length = prepare(&mut bridge);
    assert!(
        String::from_utf8_lossy(bridge.output_bytes(length as usize)).contains("\"pending\":true")
    );
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while !entered.exists() {
        assert!(
            Instant::now() < deadline,
            "native constructor did not start"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let press = |bridge: &mut Bridge<_>| {
        let kernel = bridge.host.as_ref().unwrap().runner().kernel();
        let button = kernel
            .node_by_key(kernel.find_by_test_id("increment")[0])
            .unwrap()
            .id;
        bridge.dispatch(button, 0, 0, 0.0);
    };
    press(&mut bridge);
    fs::write(&release, []).unwrap();
    loop {
        let length = prepare(&mut bridge);
        if bridge.prepared.is_some() {
            break;
        }
        let batch = String::from_utf8_lossy(bridge.output_bytes(length as usize));
        assert!(batch.contains("\"pending\":true"), "{batch}");
        assert!(
            Instant::now() < deadline,
            "native image did not become ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    press(&mut bridge);
    let length = bridge.commit_plan();
    assert!(
        String::from_utf8_lossy(bridge.output_bytes(length as usize)).contains("\"pending\":true")
    );
    assert_eq!(
        bridge.host.as_ref().unwrap().carry().slots,
        [("count".into(), Value::Number(2.0))]
    );
    prepare(&mut bridge);
    let length = bridge.commit_plan();
    assert!(
        !String::from_utf8_lossy(bridge.output_bytes(length as usize)).contains("\"error\":\"")
    );
    assert_eq!(
        bridge.host.as_ref().unwrap().carry().slots,
        [("count".into(), Value::Number(2.0))]
    );
    fs::remove_dir_all(dir).unwrap();
}
