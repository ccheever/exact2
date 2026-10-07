//! The shared Android core Contract compiled into the native archive.

#![deny(missing_docs)]

/// The ahead-of-time compiled plan, including its immutable row resource.
pub const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
/// The binary-only Android receipt, including the actual EXA1 ABI version.
pub const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

include!(concat!(env!("OUT_DIR"), "/carrier.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(rt: u32) -> Vec<u8> {
        EXACT_ANDROID.with(|r| r.borrow().get(rt).unwrap().borrow().output().to_vec())
    }

    fn input(rt: u32, text: &str) -> usize {
        exact_android::session::with_session(
            &EXACT_ANDROID,
            rt,
            |s| s.bridge.input_write(text.as_bytes()),
            || 0,
        )
    }

    fn view(rt: u32, target: &str) -> u32 {
        let n = input(
            rt,
            &format!(r#"{{"op":"tree","target":"{target}","shallow":true}}"#),
        );
        exact_android_agent(rt, n);
        let text = String::from_utf8(bytes(rt)).unwrap();
        text.split("\"roots\":[")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn exported_abi_drives_the_real_counter_input_and_batched_rows() {
        let rt = exact_android_create();
        assert_ne!(rt, 0);
        assert!(exact_android_boot(rt, 390., 844.) > 32);
        assert_eq!(&bytes(rt)[..4], b"EXA1");
        assert!(
            EXACT_ANDROID.with(|r| r.borrow().get(rt).unwrap().borrow().bridge.binary_output()),
            "the default core app must use its receipt-driven Android owner"
        );
        let increment = view(rt, "increment");
        exact_android_dispatch(rt, increment, 0, 0, 1.);
        let n = input(rt, r#"{"op":"state"}"#);
        exact_android_agent(rt, n);
        assert!(String::from_utf8(bytes(rt))
            .unwrap()
            .contains("\"count\":1"));
        let draft = view(rt, "draft");
        let n = input(rt, "Native text");
        exact_android_dispatch(rt, draft, 23, n, 2.);
        let n = input(rt, r#"{"op":"state"}"#);
        exact_android_agent(rt, n);
        assert!(String::from_utf8(bytes(rt))
            .unwrap()
            .contains("\"draft\":\"Native text\""));
        let toggle = view(rt, "toggle-batch");
        exact_android_dispatch(rt, toggle, 0, 0, 3.);
        let wire = bytes(rt);
        let count = u32::from_le_bytes(wire[8..12].try_into().unwrap());
        assert!(
            count >= 100,
            "one native transaction must carry all changed rows: {count}"
        );
        let mut position = exact_android::wire::HEADER_BYTES;
        let mut paints = 0;
        for _ in 0..count {
            let opcode = wire[position];
            let length =
                u32::from_le_bytes(wire[position + 1..position + 5].try_into().unwrap()) as usize;
            assert_ne!(
                opcode,
                exact_android::wire::JSON,
                "background-only updates stay binary: {}",
                String::from_utf8_lossy(&wire[position + 5..position + 5 + length])
            );
            if opcode == exact_android::wire::PAINT {
                paints += 1;
                assert_eq!(length, 16, "one changed light/dark color pair");
            }
            position += 5 + length;
        }
        assert_eq!(paints, 100, "every retained row receives its paint delta");
        let rows = view(rt, "rows-1000");
        exact_android_dispatch(rt, rows, 0, 0, 4.);
        let transform = view(rt, "toggle-move");
        exact_android_dispatch(rt, transform, 0, 0, 5.);
        let wire = bytes(rt);
        assert_eq!(u32::from_le_bytes(wire[8..12].try_into().unwrap()), 1);
        assert_eq!(
            wire[32], 4,
            "moving a retained subtree emits one hot record"
        );
        assert_eq!(wire[41], 1, "the hot property is translate");
        assert_eq!(wire[42], 2, "translation carries two coordinates");
        exact_android_destroy(rt);
        exact_android_destroy(rt);
        assert!(exact_android_tick(rt, 4.) > 32);
        assert!(EXACT_ANDROID.with(|r| r.borrow().get(rt).is_none()));
    }

    #[test]
    fn initial_authored_press_precedes_the_first_visible_publication() {
        let rt = exact_android_create();
        let n = input(rt, "rows-1000");
        exact_android_boot_initial(rt, 390., 844., n);
        let out = bytes(rt);
        assert_eq!(&out[..4], b"EXA1");
        assert_eq!(u32::from_le_bytes(out[28..32].try_into().unwrap()), 0);
        assert!(String::from_utf8_lossy(&out).contains("Native row 1000"));
        let n = input(rt, r#"{"op":"state"}"#);
        exact_android_agent(rt, n);
        assert!(String::from_utf8(bytes(rt))
            .unwrap()
            .contains("\"rowCount\":1000"));
        exact_android_destroy(rt);
    }

    #[test]
    fn malformed_initial_target_and_missing_press_handler_refuse_before_publication() {
        for target in ["does-not-exist", "counter", ""] {
            let rt = exact_android_create();
            let n = input(rt, target);
            exact_android_boot_initial(rt, 390., 844., n);
            let out = bytes(rt);
            assert_eq!(
                u32::from_le_bytes(out[8..12].try_into().unwrap()),
                0,
                "a refused boot cannot publish any partial tree"
            );
            assert!(String::from_utf8_lossy(&out).contains("initial press"));
            exact_android_destroy(rt);
        }
        let rt = exact_android_create();
        let len = exact_android::session::with_session(
            &EXACT_ANDROID,
            rt,
            |s| s.bridge.input_write(&[0xff]),
            || 0,
        );
        exact_android_boot_initial(rt, 390., 844., len);
        assert!(String::from_utf8_lossy(&bytes(rt)).contains("UTF-8"));
        exact_android_destroy(rt);
    }

    #[test]
    fn truncated_event_and_intrinsic_inputs_leave_the_shared_state_unchanged() {
        let rt = exact_android_create();
        exact_android_boot(rt, 390., 844.);
        let increment = view(rt, "increment");
        let n = input(rt, "x");
        exact_android_dispatch(rt, increment, 0, n + 1, 1.);
        assert!(String::from_utf8_lossy(&bytes(rt)).contains("truncated input"));
        exact_android_intrinsics(rt, 1);
        assert!(String::from_utf8_lossy(&bytes(rt)).contains("truncated record"));
        let n = input(rt, r#"{"op":"state"}"#);
        exact_android_agent(rt, n);
        assert!(String::from_utf8(bytes(rt))
            .unwrap()
            .contains("\"count\":0"));
        exact_android_destroy(rt);
    }

    #[test]
    fn core_transactions_keep_the_warm_direct_buffer_backing_and_analysis_artifacts_refuse() {
        let mut s = exact_android::session::Session::<android_core_data::Core>::default();
        s.bridge.set_compat(COMPAT);
        let n = s
            .bridge
            .boot_selected(PLAN, || android_core_data::Core, s.hooks, 390., 844.);
        s.publish(n);
        let backing = s.output().as_ptr();
        // A real repeated authored action, resolved before any agent output
        // can resize that buffer, must keep the JVM's borrowed address stable.
        let text = String::from_utf8_lossy(s.output());
        // The fixture also has navigation buttons, which replace the tree and
        // legitimately grow the output. Select the repeated counter action.
        let marker = "\"testId\":\"increment\"";
        let start = text.find(marker).unwrap();
        let id_start = text[..start].rfind("\"id\":").unwrap() + 5;
        let id: u32 = text[id_start..].split(',').next().unwrap().parse().unwrap();
        for now in 1..10 {
            let n = s.bridge.dispatch(id, 0, 0, f64::from(now));
            s.publish(n);
            assert_eq!(s.output().as_ptr(), backing);
        }
        let mut refused = exact_android::session::Session::<android_core_data::Core>::default();
        refused
            .bridge
            .set_compat(r#"{"embedded":{"analysis":true}}"#);
        let n = refused.bridge.boot_selected(
            PLAN,
            || android_core_data::Core,
            refused.hooks,
            390.,
            844.,
        );
        refused.publish(n);
        assert_eq!(
            u32::from_le_bytes(refused.output()[8..12].try_into().unwrap()),
            0
        );
        assert!(
            String::from_utf8_lossy(refused.output()).contains("compatibility analysis artifact")
        );
    }
}
