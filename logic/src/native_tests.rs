//! Real shared-library fixture; exercises the C seam and private retained loading.
use super::*;
use std::{fs, process::Command};

#[test]
fn native_library_uses_copied_buffers_and_keeps_existing_sessions_alive() {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "exact-logic-test-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    let source = dir.join("fixture.c");
    let entered = dir.join("entered");
    let release = dir.join("release");
    let library = dir.join(if cfg!(target_os = "macos") {
        "fixture.dylib"
    } else {
        "fixture.so"
    });
    let mut encoded = exact_plan::bytes::Writer::default();
    encoded.u32(abi::ABI);
    encoded.u8(2);
    encoded.u8(0);
    encoded.u32(0);
    encoded.u8(0);
    Value::Number(99.).encode(&mut encoded);
    let answer = encoded.into_vec();
    let encode = |op: u8, body: &dyn Fn(&mut exact_plan::bytes::Writer)| {
        let mut writer = exact_plan::bytes::Writer::default();
        writer.u32(abi::ABI);
        writer.u8(op);
        body(&mut writer);
        writer.into_vec()
    };
    let metadata = encode(0, &|w| {
        w.string("test.logic");
        w.string("secret.keep token");
    });
    let unit = encode(1, &|w| {
        w.u8(0);
        Value::Unit.encode(w);
    });
    let c_bytes = |bytes: &[u8]| {
        bytes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let metadata = c_bytes(&metadata);
    let unit = c_bytes(&unit);

    let version = abi::ABI;
    let result = answer
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    fs::write(&source,format!(r#"
        #include <stdint.h>
        #include <stdlib.h>
        #include <string.h>
        #include <stdio.h>
        #include <unistd.h>
        __attribute__((constructor)) static void prepare(void) {{
            FILE *f = fopen("{entered}", "w"); if (f) fclose(f);
            while (access("{release}", F_OK) != 0) usleep(1000);
        }}
        typedef struct {{ uint8_t output[128]; uint32_t length; }} Session;
        uint32_t exact_logic_stateless(void) {{ return 1; }}
        uint32_t exact_logic_abi(void) {{ return {version}; }}
        uintptr_t exact_logic_create(void) {{ return (uintptr_t)calloc(1,sizeof(Session)); }}
        void exact_logic_destroy(uintptr_t session) {{ free((void*)session); }}
        uintptr_t exact_logic_alloc(uint32_t len) {{ return (uintptr_t)malloc(len); }}
        void exact_logic_dealloc(uintptr_t ptr,uint32_t len) {{ free((void*)ptr); }}
        uint32_t exact_logic_call(uintptr_t session,uintptr_t ptr,uint32_t len) {{
            uint8_t metadata[] = {{{metadata}}}, unit[] = {{{unit}}}, answer[] = {{{result}}};
            uint8_t op = ((uint8_t*)ptr)[4];
            uint8_t *bytes = op == 0 ? metadata : op < 3 ? unit : answer;
            uint32_t length = op == 0 ? sizeof(metadata) : op < 3 ? sizeof(unit) : sizeof(answer);
            memcpy(((Session*)session)->output,bytes,length);
            ((Session*)session)->length = length; return 0;
        }}
        uintptr_t exact_logic_output(uintptr_t session) {{ return (uintptr_t)((Session*)session)->output; }}
        uint32_t exact_logic_output_len(uintptr_t session) {{ return ((Session*)session)->length; }}
    "#, entered=entered.display(), release=release.display())).unwrap();
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
    let bytes = fs::read(&library).unwrap();
    assert!(!crate::native::preload(&bytes).unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    while !entered.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "native loader did not enter the fixture"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // The worker is deliberately held inside dlopen. Readiness and an
    // unrelated admission must still return, without taking its loader lock.
    assert!(!crate::native::preload(&bytes).unwrap());
    assert!(!crate::native::preload(b"queued invalid image").unwrap());
    assert_eq!(crate::native::mapping_count(&bytes), 1);
    // Execute the new Wasm even while a real native constructor is blocked.
    let portable = crate::tests::wasm_with_contract("", true);
    let pair = |wasm: &[u8], native: &[u8]| {
        let mut bytes = b"EXLT\x01\0\0\0".to_vec();
        bytes.extend_from_slice(&(wasm.len() as u32).to_le_bytes());
        bytes.extend_from_slice(wasm);
        bytes.extend_from_slice(native);
        bytes
    };
    let mut tiered = crate::tiered::load(&pair(&portable, &bytes)).unwrap();
    assert!(
        crate::tiered::load(&pair(&crate::tests::wasm_with_contract("", false), &bytes)).is_err()
    );
    let mut store = Store::new("secret.keep token", []);
    let input = abi::call_request(&store, "answer", &[], None).unwrap();
    abi::unit_reply(&tiered.call(&abi::activate_request()).unwrap()).unwrap();
    assert_eq!(
        abi::call_reply(&tiered.call(&input).unwrap(), &mut store).unwrap(),
        Answer::Now(Value::Number(42.))
    );
    assert_eq!(store.take_writes().len(), 1);
    // Superseded tiered sessions never queue behind the blocked load.
    let obsolete = b"superseded tiered candidate";
    drop(crate::tiered::load(&pair(&portable, obsolete)).unwrap());
    assert_eq!(crate::native::mapping_count(obsolete), 0);
    fs::write(&release, b"ready").unwrap();
    while !crate::native::preload(&bytes).unwrap() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Promotion dispatches the next call exactly once, retaining the same
    // Store and never replaying the earlier Wasm mutation.
    assert_eq!(
        abi::call_reply(&tiered.call(&input).unwrap(), &mut store).unwrap(),
        Answer::Now(Value::Number(99.))
    );
    assert!(store.take_writes().is_empty());
    assert_eq!(store.snapshot(), vec![("token".into(), "new".into())]);
    assert_eq!(crate::native::mapping_count(obsolete), 0);
    let mut first = crate::native::load(&bytes).unwrap();
    let mut second = crate::native::load(&bytes).unwrap();
    // More sessions than the artifact budget still use one mapped image.
    for _ in 0..70 {
        drop(crate::native::load(&bytes).unwrap());
    }
    assert_eq!(crate::native::mapping_count(&bytes), 1);
    fs::remove_dir_all(&dir).unwrap();
    let input = abi::call_request(&Store::default(), "answer", &[], None).unwrap();
    let first_bytes = first.call(&input).unwrap();
    let second_bytes = second.call(&input).unwrap();
    drop(second);
    assert_eq!(
        abi::call_reply(&first_bytes, &mut Store::default()).unwrap(),
        Answer::Now(Value::Number(99.))
    );
    assert_eq!(first.call(&input).unwrap(), second_bytes);

    let refused = b"not a shared library";
    let first_error = crate::native::load(refused).err().unwrap();
    for _ in 0..70 {
        assert_eq!(crate::native::load(refused).err().unwrap(), first_error);
    }
    assert_eq!(crate::native::mapping_count(refused), 1);
    // A refused native image and an identity mismatch both retain the new Wasm.
    let mut fallback = crate::tiered::load(&pair(&portable, refused)).unwrap();
    abi::unit_reply(&fallback.call(&abi::activate_request()).unwrap()).unwrap();
    assert_eq!(
        abi::call_reply(&fallback.call(&input).unwrap(), &mut store).unwrap(),
        Answer::Now(Value::Number(42.))
    );
    let mut other = portable.clone();
    let offset = other.windows(10).position(|w| w == b"test.logic").unwrap();
    other[offset..offset + 10].copy_from_slice(b"test.other");
    let mut mismatch = crate::tiered::load(&pair(&other, &bytes)).unwrap();
    abi::unit_reply(&mismatch.call(&abi::activate_request()).unwrap()).unwrap();
    assert_eq!(
        abi::call_reply(&mismatch.call(&input).unwrap(), &mut store).unwrap(),
        Answer::Now(Value::Number(42.))
    );
    for invalid in [vec![], b"EXLT\x01\0\0\0".to_vec(), pair(&portable, &[])] {
        assert!(crate::tiered::load(&invalid).is_err());
    }
}
