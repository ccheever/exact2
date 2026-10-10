//! Actual Apple boundary: generation-safe pair admission and receipt order.
//! Fixtures mirror the frozen120-byte Web protocol; no Swift/DOM emulation.
use super::Host;
use crate::abi::{Bridge, Hooks};
use exact_kernel::MonospaceMeasurer;
use exact_motion::{HoldEnd, Property, Value as MotionValue};
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const SOURCE: &str = r#"component App
  state zoom = 1
  state x = 0
  state y = 0
  state released = 0
  state measured = 0
  state bw = 0
  state shown = true
  state reference = "photo"
  action geometry(w: number, h: number, pw: number, ph: number)
    measured = measured + 1
    bw = w
  action finish(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    released = released + 1
    x = px
    y = py
  action zoomIn
    zoom = 2
  action unbind
    reference = "absent"
  action remove
    shown = false
  view
    column
      button testId="zoom" press=zoomIn width=80 height=24
      button testId="unbind" press=unbind width=80 height=24
      button testId="remove" press=remove width=80 height=24
      when shown
        column testId="clip" width=320.25 height=200.5 overflow="hidden" padding=0 border-width=0
          column id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 translate=`${x}px ${y}px` scale=zoom transition="translate -exact-spring(180, 12, 1), scale -exact-spring(180, 12, 1)"
            column testId="handle" transformDragFor=reference transformgeometry=geometry transformrelease=finish
      text `${released}/${measured}/${bw}` testId="result"
"#;

// v2 LE: version/op, runtime/handle/target/clip/geometry/T/S u64,
// six scalar f64 payload slots and one clock-ms. Nothing crosses as a JS Number.
#[derive(Clone, Copy)]
struct Packet {
    op: u32,
    identity: [u64; 5],
    tokens: [u64; 2],
    values: [f64; 6],
    now: f64,
}
impl Packet {
    fn bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(120);
        for word in [2u32, self.op] {
            bytes.extend(word.to_le_bytes());
        }
        for word in self.identity.into_iter().chain(self.tokens) {
            bytes.extend(word.to_le_bytes());
        }
        for value in self.values.into_iter().chain([self.now]) {
            bytes.extend(value.to_le_bytes());
        }
        assert_eq!(bytes.len(), 120);
        bytes
    }
    fn send(self, host: &mut Host<NoData>) -> String {
        host.transform_motion(&self.bytes())
    }
}
fn quoted(json: &str, field: &str) -> u64 {
    json.split(&format!("\"{field}\":\""))
        .nth(1)
        .unwrap_or_else(|| panic!("missing {field}: {json}"))
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn boot_source(source: &str) -> (Host<NoData>, Packet, String) {
    boot_plan(&contract::compile(source).unwrap().encode())
}
/// `source` compiled, with the handle's constant `testId` rewritten to trap
/// once `x` passes 1e39: a failure while the tree changes, after an action
/// wrote `x`. (Since 2026-09-22 a bound value its row refuses, such as a
/// `translate` past f32, is unset rather than failing the commit.)
fn boot_trapping(source: &str) -> (Host<NoData>, Packet, String) {
    use exact_plan::{asm::Asm, builder::PlanBuilder, Opcode, SlotsId, StrId};
    let plan = contract::compile(source).unwrap();
    let x = plan
        .slots
        .iter()
        .position(|s| plan.str(s.name) == "x")
        .unwrap();
    let handle = plan
        .bindings
        .iter()
        .position(|b| {
            let code = plan.code(b.expr);
            code.len() == 6
                && code[0] == Opcode::Str as u8
                && plan.str(StrId(u32::from_le_bytes(code[1..5].try_into().unwrap()))) == "handle"
        })
        .expect("the handle's testId");
    let mut b = PlanBuilder::from_plan(plan);
    let name = b.str("handle");
    let mut guard = Asm::new();
    let fits = guard.label();
    guard
        .load_slot(SlotsId(x as u32))
        .number(1e39)
        .simple(Opcode::Gt)
        .jump_if_false(fits)
        .simple(Opcode::Unit)
        .simple(Opcode::Unwrap)
        .place(fits)
        .str(name);
    let guard = b.code(guard);
    let mut plan = b.finish().unwrap();
    plan.bindings[handle].expr = guard;
    plan.validate().unwrap();
    boot_plan(&plan.encode())
}
fn boot_plan(plan: &[u8]) -> (Host<NoData>, Packet, String) {
    let (host, batch) = Host::boot(
        plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        800.,
    )
    .unwrap();
    let binding = batch
        .split("\"op\":\"transform-drag\"")
        .nth(1)
        .unwrap_or_else(|| panic!("missing transform binding: {batch}"));
    let packet = Packet {
        op: 10,
        identity: [
            quoted(binding, "runtime"),
            quoted(binding, "handleKey"),
            quoted(binding, "targetKey"),
            quoted(binding, "clipKey"),
            1,
        ],
        tokens: [0; 2],
        values: [320.25, 200.5, 320.25, 200.5, 0.0, 0.0],
        now: 0.0,
    };
    (host, packet, batch)
}
fn boot() -> (Host<NoData>, Packet, String) {
    boot_source(SOURCE)
}
fn accepted(reply: &str) {
    assert!(reply.contains("\"accepted\":true"), "{reply}");
    assert!(!reply.contains("\"error\":\""), "{reply}");
}
fn id(host: &Host<NoData>, test_id: &str) -> u32 {
    let kernel = host.runner().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(test_id)[0])
        .unwrap()
        .id
}
fn press(host: &mut Host<NoData>, test_id: &str, now: f64) -> String {
    host.dispatch_at(id(host, test_id), Event::Press, now)
}
fn held() -> (Host<NoData>, Packet) {
    let (mut host, mut p, _) = boot();
    accepted(&p.send(&mut host));
    p.op = 11;
    p.values = [13.25, -7.5, 1.375, 0.0, 0.0, 0.0];
    p.now = 100.0;
    let reply = p.send(&mut host);
    accepted(&reply);
    assert!(reply.contains("\"value\":[13.25,-7.5,1.375]"), "{reply}");
    assert_eq!(quoted(&reply, "runtime"), p.identity[0]);
    assert_eq!(quoted(&reply, "geometrySequence"), 1);
    p.tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 12;
    (host, p)
}
fn current(host: &Host<NoData>, node: u64) -> [MotionValue; 2] {
    [Property::Translate, Property::Scale]
        .map(|property| host.engine().value(node, property).unwrap())
}
fn count(host: &Host<NoData>, slot: &str) -> f64 {
    let Some(Value::Number(value)) = host.runner().slot(slot) else {
        panic!("missing numeric slot {slot}")
    };
    *value
}

#[test]
fn binding_follows_complete_tree_and_fractional_geometry_is_deduplicated() {
    let (mut host, mut p, batch) = boot();
    assert!(
        batch.find("\"op\":\"roots\"").unwrap() < batch.find("\"op\":\"transform-drag\"").unwrap()
    );
    accepted(&p.send(&mut host));
    assert_eq!(count(&host, "measured"), 1.0);
    assert_eq!(count(&host, "bw"), 320.25);
    accepted(&p.send(&mut host));
    p.identity[4] += 1; // Clip origin moved; dimensions did not.
    accepted(&p.send(&mut host));
    assert_eq!(count(&host, "measured"), 1.0);
    p.values[0] = 320.5;
    assert!(p.send(&mut host).contains("\"error\":\"")); // Same sequence cannot change facts.
    assert_eq!(count(&host, "bw"), 320.25);
}

#[test]
fn begin_requires_positive_current_geometry_and_invalidations_do_not_fake_actions() {
    let (mut host, mut p, _) = boot();
    p.op = 11;
    p.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    assert!(p.send(&mut host).contains("\"accepted\":false"));
    p.op = 10;
    p.values = [0.0; 6];
    accepted(&p.send(&mut host));
    assert_eq!(count(&host, "measured"), 1.0);
    p.op = 11;
    p.values[2] = 1.0;
    assert!(p.send(&mut host).contains("\"accepted\":false"));
    p.op = 14;
    p.identity[4] += 1;
    p.values = [0.0; 6];
    accepted(&p.send(&mut host));
    assert_eq!(count(&host, "measured"), 1.0);
    assert_eq!(host.engine().now(), 0.0);
}

#[test]
fn full_live_payload_preflight_preserves_both_samples_tokens_and_clock() {
    let (mut host, p) = held();
    let before = current(&host, p.identity[2]);
    let time = host.engine().now();
    for op in [11, 12, 13] {
        for field in 0..7 {
            let mut bad = Packet {
                op,
                now: 200.0,
                ..p
            };
            if op == 11 {
                bad.tokens = [0; 2];
            }
            if field == 6 {
                bad.now = f64::NAN;
            } else {
                bad.values[field] = f64::NAN;
            }
            assert!(
                bad.send(&mut host).contains("\"error\":\""),
                "op {op}, field {field}"
            );
            assert_eq!(current(&host, p.identity[2]), before);
            assert_eq!(host.engine().now(), time);
            assert!(p.tokens.into_iter().all(|token| host.has_hold(token)));
            assert_eq!(count(&host, "released"), 0.0);
        }
    }
    for scale in [0.0, -1.0, 1e-100, f64::INFINITY, 1e39] {
        let mut bad = Packet {
            op: 13,
            now: 200.0,
            ..p
        };
        bad.values[2] = scale;
        assert!(bad.send(&mut host).contains("\"error\":\""));
        assert_eq!(current(&host, p.identity[2]), before);
        assert_eq!(host.engine().now(), time);
    }
    for now in [-1.0, 99.0, f64::INFINITY] {
        assert!(Packet { op: 13, now, ..p }
            .send(&mut host)
            .contains("\"error\":\""));
        assert_eq!(current(&host, p.identity[2]), before);
        assert_eq!(host.engine().now(), time);
    }
}

#[test]
fn each_stale_identity_and_geometry_stamp_refuses_before_bad_values_or_future_time() {
    let (mut host, p) = held();
    for field in 0..5 {
        let mut stale = Packet {
            op: 13,
            now: 1e9,
            values: [f64::NAN; 6],
            ..p
        };
        stale.identity[field] ^= 1 << 60;
        assert!(
            stale.send(&mut host).contains("\"accepted\":false"),
            "field {field}"
        );
        assert_eq!(host.engine().now(), 0.1);
        assert!(p.tokens.into_iter().all(|token| host.has_hold(token)));
        assert_eq!(count(&host, "released"), 0.0);
    }
}

#[test]
fn every_unused_slot_refuses_before_mutation_including_duplicate_geometry() {
    let (mut host, p) = held();
    let before = current(&host, p.identity[2]);
    for op in 10..=14 {
        let mut input = Packet {
            op,
            now: 200.0,
            ..p
        };
        if [10, 11, 14].contains(&op) {
            input.tokens = [0; 2];
            for token in 0..2 {
                let mut bad = input;
                bad.tokens[token] = p.tokens[token];
                assert!(
                    bad.send(&mut host).contains("\"error\":\""),
                    "op {op}, token {token}"
                );
            }
        }
        let unused = match op {
            10 => {
                input.values = [320.25, 200.5, 320.25, 200.5, 0.0, 0.0];
                4..6
            }
            11 | 12 => 3..6,
            13 => 6..6,
            _ => {
                input.values = [0.0; 6];
                0..6
            }
        };
        for slot in unused {
            for invalid in [1.0, f64::NAN] {
                let mut bad = input;
                bad.values[slot] = invalid;
                assert!(
                    bad.send(&mut host).contains("\"error\":\""),
                    "op {op}, slot {slot}"
                );
            }
        }
        assert_eq!(current(&host, p.identity[2]), before);
        assert_eq!(host.engine().now(), 0.1);
        assert!(p.tokens.into_iter().all(|token| host.has_hold(token)));
        assert_eq!(count(&host, "measured"), 1.0);
        assert_eq!(count(&host, "released"), 0.0);
    }
}

#[test]
fn geometry_retarget_and_new_none_declaration_arrive_while_held_before_cancellation() {
    let source = SOURCE
        .replace("writes measured, bw\n", "writes measured, bw, x, zoom\n")
        .replace("    bw = w\n", "    bw = w\n    if w > 400\n      x = 99\n      zoom = 2\n")
        .replace("transition=\"translate -exact-spring(180, 12, 1), scale -exact-spring(180, 12, 1)\"", "transition=(bw > 400 ? \"none\" : \"translate -exact-spring(180, 12, 1), scale -exact-spring(180, 12, 1)\")");
    let (mut host, mut p, _) = boot_source(&source);
    accepted(&p.send(&mut host));
    p.op = 11;
    p.now = 100.0;
    p.values = [13.25, -7.5, 1.375, 0.0, 0.0, 0.0];
    let begun = p.send(&mut host);
    accepted(&begun);
    let tokens = [
        quoted(&begun, "translateToken"),
        quoted(&begun, "scaleToken"),
    ];
    p.op = 10;
    p.identity[4] = 2;
    p.now = 200.0;
    p.values = [401.25, 200.5, 401.25, 200.5, 0.0, 0.0];
    accepted(&p.send(&mut host));
    assert_eq!(
        current(&host, p.identity[2]),
        [MotionValue::new(99.0, 0.0), MotionValue::scalar(2.0)]
    );
    assert_eq!(host.engine().now(), 0.2);
    assert!(tokens.into_iter().all(|token| !host.has_hold(token)));
    for prop in [Property::Translate, Property::Scale] {
        assert!(host
            .engine()
            .spring_descriptor(p.identity[2], prop)
            .is_none());
    }
    assert_eq!(count(&host, "measured"), 2.0);
    assert_eq!(count(&host, "released"), 0.0);
}

#[test]
fn zoom_action_changes_latest_target_but_not_pair_presentation_and_release_fires_once() {
    let (mut host, mut p) = held();
    let before = current(&host, p.identity[2]);
    press(&mut host, "zoom", 105.0);
    assert_eq!(
        host.runner()
            .kernel()
            .node(id(&host, "photo"))
            .unwrap()
            .style
            .scale,
        2.0
    );
    assert_eq!(current(&host, p.identity[2]), before);
    p.op = 13;
    p.values = [23.75, -8.25, 1.375, 30.0, -10.0, 0.0];
    p.now = 110.0;
    assert!(
        p.send(&mut host)
            .contains("unused transform values must be zero"),
        "op 13's velocities are the engine's, never the packet's"
    );
    p.values = [23.75, -8.25, 1.375, 0.0, 0.0, 0.0];
    let reply = p.send(&mut host);
    accepted(&reply);
    let measured: Vec<f64> = reply
        .split("\"velocity\":[")
        .nth(1)
        .and_then(|v| v.split(']').next())
        .unwrap_or_else(|| panic!("{reply}"))
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect();
    // Begin at 100 ms, terminal at 110 ms: 10.5 px and -0.75 px in 10 ms.
    assert!((measured[0] - 1050.0).abs() < 1e-6, "{measured:?}");
    assert!((measured[1] + 75.0).abs() < 1e-6, "{measured:?}");
    assert_eq!(measured[2], 0.0);
    assert!(reply.contains("\"dispatched\":true"), "{reply}");
    assert!(reply.contains("\"committed\":true"), "{reply}");
    assert_eq!(count(&host, "released"), 1.0);
    assert!(p.tokens.into_iter().all(|token| host.has_hold(token)));
    assert_eq!(
        current(&host, p.identity[2]),
        [MotionValue::new(23.75, -8.25), MotionValue::scalar(1.375)]
    );
    assert!(p.send(&mut host).contains("\"accepted\":false"));
    for (token, velocity) in p
        .tokens
        .into_iter()
        .zip([MotionValue::new(30.0, -10.0), MotionValue::ZERO])
    {
        let batch = host.hold_end(token, HoldEnd::Release { velocity }, 110.0);
        assert!(batch.contains("\"error\":null"), "{batch}");
        assert!(!host.has_hold(token));
    }
    assert_eq!(count(&host, "released"), 1.0);
    assert_eq!(
        host.engine()
            .spring_descriptor(p.identity[2], Property::Scale)
            .unwrap()
            .target,
        MotionValue::scalar(2.0)
    );
}

#[test]
fn replacing_one_property_cleans_old_survivor_without_cancelling_successor() {
    let (mut host, mut p) = held();
    let target = id(&host, "photo");
    let batch = host.hold_begin(target, Property::Scale, 110.0);
    let replacement = quoted(&batch, "token");
    host.hold_update(replacement, MotionValue::scalar(1.8), 110.0);
    p.now = 1e9;
    p.values = [f64::NAN; 6];
    assert!(p.send(&mut host).contains("\"accepted\":false"));
    assert!(host.has_hold(replacement));
    assert!(!host.has_hold(p.tokens[0]));
    assert_eq!(host.engine().now(), 0.11);
    assert_eq!(current(&host, p.identity[2])[1], MotionValue::scalar(1.8));
    assert_eq!(count(&host, "released"), 0.0);
}

#[test]
fn receipt_retirement_is_token_qualified_precedes_lowering_and_uses_receipt_clock() {
    let (mut host, p) = held();
    let reply = press(&mut host, "unbind", 125.0);
    for token in p.tokens {
        assert!(reply.contains(&format!("\"token\":\"{token}\"")), "{reply}");
        assert!(!host.has_hold(token));
    }
    assert!(
        reply.find("\"op\":\"retire-motion\"").unwrap() < reply.find("\"op\":\"present\"").unwrap()
    );
    assert_eq!(
        host.engine()
            .spring_descriptor(p.identity[2], Property::Scale)
            .unwrap()
            .start,
        0.125
    );
    assert_eq!(count(&host, "released"), 0.0);
}

#[test]
fn new_mapping_sequence_cancels_hold_even_with_unchanged_dimensions() {
    let (mut host, p) = held();
    let geometry = Packet {
        op: 10,
        identity: [
            p.identity[0],
            p.identity[1],
            p.identity[2],
            p.identity[3],
            2,
        ],
        tokens: [0; 2],
        values: [320.25, 200.5, 320.25, 200.5, 0.0, 0.0],
        now: 110.0,
    };
    accepted(&geometry.send(&mut host));
    assert!(p.tokens.into_iter().all(|token| !host.has_hold(token)));
    assert_eq!(count(&host, "measured"), 1.0);
    assert_eq!(count(&host, "released"), 0.0);
    assert!(p.send(&mut host).contains("\"accepted\":false"));
}

#[test]
fn old_runtime_packet_cannot_attach_to_identical_new_tree() {
    let (mut old, p, _) = boot();
    accepted(&p.send(&mut old));
    let (mut new, other, _) = boot();
    assert_ne!(p.identity[0], other.identity[0]);
    assert_eq!(&p.identity[1..4], &other.identity[1..4]);
    assert!(p.send(&mut new).contains("\"accepted\":false"));
    assert_eq!(count(&new, "measured"), 0.0);
}

#[test]
fn action_refusal_reports_failed_kernel_commit_without_claiming_runner_slot_rollback() {
    let source = SOURCE.replace(
        "    x = px\n",
        "    x = 10000000000000000000000000000000000000000\n",
    );
    let (mut host, mut p, _) = boot_trapping(&source);
    accepted(&p.send(&mut host));
    p.op = 11;
    p.now = 100.0;
    p.values = [13.25, -7.5, 1.375, 0.0, 0.0, 0.0];
    let reply = p.send(&mut host);
    accepted(&reply);
    p.tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 13;
    let reply = p.send(&mut host);
    assert!(reply.contains("\"accepted\":true"), "{reply}");
    assert!(reply.contains("\"dispatched\":true"), "{reply}");
    assert!(reply.contains("\"committed\":false"), "{reply}");
    assert!(reply.contains("\"error\":\""), "{reply}");
    // Existing Runner fail-stop semantics keep already-written action slots.
    // The Apple result must name failed kernel publication, not promise rollback.
    assert_eq!(count(&host, "released"), 1.0);
    assert_eq!(count(&host, "x"), 1e40);
    assert_eq!(
        host.runner()
            .kernel()
            .node(id(&host, "photo"))
            .unwrap()
            .style
            .translate
            .x,
        0.0
    );
    assert!(p.tokens.into_iter().all(|token| host.has_hold(token)));
    assert!(p.send(&mut host).contains("\"accepted\":false"));
}

#[test]
fn release_action_can_destroy_both_holds_and_late_cleanup_is_inert() {
    let source = SOURCE
        .replace("writes released, x, y\n", "writes released, x, y, shown\n")
        .replace("    y = py\n", "    y = py\n    shown = false\n");
    let (mut host, mut p, _) = boot_source(&source);
    accepted(&p.send(&mut host));
    p.op = 11;
    p.now = 100.0;
    p.values = [13.25, -7.5, 1.375, 0.0, 0.0, 0.0];
    let reply = p.send(&mut host);
    accepted(&reply);
    p.tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 13;
    accepted(&p.send(&mut host));
    assert_eq!(count(&host, "released"), 1.0);
    for token in p.tokens {
        let out = host.hold_end(token, HoldEnd::Cancel, 1e9);
        assert!(out.contains("\"error\":null"), "{out}");
        assert!(!host.has_hold(token));
    }
    assert_eq!(host.engine().now(), 0.1);
    assert!(p.send(&mut host).contains("\"accepted\":false"));
}

#[test]
fn apple_bridge_preserves_high_sequence_and_exact_packet_length() {
    let mut bridge = Bridge::new();
    let n = bridge.boot(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        Hooks::none(),
        400.0,
        800.0,
    );
    let batch = String::from_utf8(bridge.output_bytes(n as usize).to_vec()).unwrap();
    let binding = batch.split("\"op\":\"transform-drag\"").nth(1).unwrap();
    let p = Packet {
        op: 10,
        identity: [
            quoted(binding, "runtime"),
            quoted(binding, "handleKey"),
            quoted(binding, "targetKey"),
            quoted(binding, "clipKey"),
            9007199254740993,
        ],
        tokens: [0; 2],
        values: [320.25, 200.5, 320.25, 200.5, 0.0, 0.0],
        now: 0.0,
    };
    let bytes = p.bytes();
    bridge.input_write(&bytes);
    let n = bridge.transform_motion(bytes.len());
    accepted(std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap());
    for len in [0, 48, 119, 121] {
        bridge.input_write(&bytes);
        let n = bridge.transform_motion(len);
        let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(out.contains("\"error\":\""), "{len}: {out}");
    }
    let mut begin = Packet {
        op: 11,
        values: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        ..p
    };
    bridge.input_write(&begin.bytes());
    let n = bridge.transform_motion(120);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    accepted(out);
    assert_eq!(quoted(out, "geometrySequence"), 9007199254740993);
    begin.tokens = [quoted(out, "translateToken"), quoted(out, "scaleToken")];
    assert_ne!(begin.tokens[0], begin.tokens[1]);
}

#[test]
fn one_target_multiple_handles_refuses_second_target_until_owner_disappears() {
    let source=SOURCE.replace("      text `${released}", "      column testId=\"clip-other\" width=320.25 height=200.5 overflow=\"hidden\" padding=0 border-width=0\n        column id=\"other-photo\" testId=\"other-photo\" width=\"100%\" height=\"100%\" box-sizing=\"border-box\" padding=0 border-width=0\n          column testId=\"other-handle\" transformDragFor=\"other-photo\" transformgeometry=geometry transformrelease=finish\n      text `${released}")
        .replace("            column testId=\"handle\" transformDragFor=reference transformgeometry=geometry transformrelease=finish", "            column testId=\"handle\" transformDragFor=reference transformgeometry=geometry transformrelease=finish\n            column testId=\"handle-two\" transformDragFor=reference transformgeometry=geometry transformrelease=finish");
    let (mut host, mut p, batch) = boot_source(&source);
    let binding_packets: Vec<_> = batch.split("\"op\":\"transform-drag\"").skip(1).collect();
    assert_eq!(binding_packets.len(), 3);
    assert!(binding_packets[0].contains("\"targetKey\":\""));
    assert!(binding_packets[1].contains("\"targetKey\":\""));
    assert!(
        binding_packets[2].contains("\"target\":null"),
        "{}",
        binding_packets[2]
    );
    let other_handle = host.runner().kernel().find_by_test_id("other-handle")[0];
    let other = host
        .runner()
        .kernel()
        .transform_drag_binding(other_handle)
        .unwrap();
    let mut refused = Packet {
        identity: [
            p.identity[0],
            exact_kernel::motion::motion_node(other.handle),
            exact_kernel::motion::motion_node(other.target),
            exact_kernel::motion::motion_node(other.clip),
            1,
        ],
        ..p
    };
    assert!(refused.send(&mut host).contains("\"accepted\":false"));
    refused.op = 11;
    refused.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    assert!(refused.send(&mut host).contains("\"accepted\":false"));
    accepted(&p.send(&mut host));
    p.op = 11;
    p.values = [13.25, -7.5, 1.375, 0.0, 0.0, 0.0];
    let first = p.send(&mut host);
    accepted(&first);
    let old = [
        quoted(&first, "translateToken"),
        quoted(&first, "scaleToken"),
    ];
    p.identity[1] =
        exact_kernel::motion::motion_node(host.runner().kernel().find_by_test_id("handle-two")[0]);
    p.op = 10;
    p.values = [320.25, 200.5, 320.25, 200.5, 0.0, 0.0];
    accepted(&p.send(&mut host));
    p.op = 11;
    p.values = [23.0, 2.0, 1.5, 0.0, 0.0, 0.0];
    let second = p.send(&mut host);
    accepted(&second);
    assert!(old.into_iter().all(|token| !host.has_hold(token)));
    let old = [
        quoted(&second, "translateToken"),
        quoted(&second, "scaleToken"),
    ];
    let removed = press(&mut host, "remove", 100.0);
    assert!(
        removed.contains(&format!("\"targetKey\":\"{}\"", refused.identity[2])),
        "{removed}"
    );
    assert!(old.into_iter().all(|token| !host.has_hold(token)));
    refused.op = 10;
    refused.now = 100.0;
    refused.values = [320.25, 200.5, 320.25, 200.5, 0.0, 0.0];
    accepted(&refused.send(&mut host));
    refused.op = 11;
    refused.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    accepted(&refused.send(&mut host));
}

#[test]
fn geometry_action_failure_cannot_leave_previous_pair_alive() {
    let source = SOURCE
        .replace("writes measured, bw\n", "writes measured, bw, x\n")
        .replace(
            "    bw = w\n",
            "    bw = w\n    if w > 400\n      x = 10000000000000000000000000000000000000000\n",
        );
    let (mut host, mut p, _) = boot_trapping(&source);
    accepted(&p.send(&mut host));
    p.op = 11;
    p.now = 100.0;
    p.values = [13.25, -7.5, 1.375, 0.0, 0.0, 0.0];
    let reply = p.send(&mut host);
    accepted(&reply);
    let tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 10;
    p.identity[4] = 2;
    p.now = 200.0;
    p.values = [401.25, 200.5, 401.25, 200.5, 0.0, 0.0];
    let reply = p.send(&mut host);
    assert!(reply.contains("\"error\":\""), "{reply}");
    assert!(
        tokens.into_iter().all(|token| !host.has_hold(token)),
        "old pair survived failed geometry callback: {reply}"
    );
    assert_eq!(host.engine().now(), 0.2);
    assert_eq!(count(&host, "released"), 0.0);
    for token in tokens {
        assert!(reply.contains(&format!("\"token\":\"{token}\"")), "{reply}");
    }
}

#[test]
fn rejected_packets_keep_unrelated_motion_and_timer_status() {
    let source = SOURCE.replace("    column\n", "    column testId=\"root\" transition=\"opacity 1000ms linear\"\n")
        .replace("  view\n", "  state ticks = 0\n  action tick\n    ticks = ticks + 1\n  task ticker mount\n    every(1000, tick)\n  view\n");
    let (mut host, mut p, _) = boot_source(&source);
    accepted(&p.send(&mut host));
    p.op = 11;
    p.values = [12., -5., 1.4, 0., 0., 0.];
    let begun = p.send(&mut host);
    accepted(&begun);
    let tokens = [
        quoted(&begun, "translateToken"),
        quoted(&begun, "scaleToken"),
    ];
    let root = id(&host, "root");
    let reply = host.hold_begin(root, Property::Opacity, 0.);
    let opacity = quoted(&reply, "token");
    host.hold_update(opacity, MotionValue::scalar(0.3), 0.);
    host.hold_end(opacity, HoldEnd::Cancel, 0.);
    let before = format!("{:?}", host.engine());
    for bytes in [vec![], vec![0; 119], vec![0; 121], vec![0; 120]] {
        let reply = host.transform_motion(&bytes);
        assert!(reply.contains("\"motion\":true"), "{reply}");
        assert!(reply.contains("\"timers\":true"), "{reply}");
        assert!(reply.contains("\"error\":\""), "{reply}");
        assert_eq!(format!("{:?}", host.engine()), before);
        assert!(tokens.iter().all(|&t| host.has_hold(t)));
    }
}

#[test]
fn native_ancestor_active_curve_refuses_before_clock_even_at_identity_sample() {
    let (mut host, mut p, _) = boot();
    accepted(&p.send(&mut host));
    let ancestor = host
        .runner()
        .kernel()
        .node(id(&host, "clip"))
        .unwrap()
        .parent
        .unwrap();
    let held = host.hold_begin(ancestor, Property::Translate, 0.);
    let held = quoted(&held, "token");
    // A held identity transform is still an unsupported active ancestor owner.
    p.op = 11;
    p.now = 1e9;
    p.values = [f64::NAN; 6];
    let reply = p.send(&mut host);
    assert!(reply.contains("\"accepted\":false"), "{reply}");
    assert!(!reply.contains("\"error\":\""), "{reply}");
    assert_eq!(host.engine().now(), 0.);
    assert!(host.has_hold(held));
}

#[test]
fn repeated_takeovers_and_independent_terminal_ends_retain_only_current_pair() {
    let (mut host, mut p, _) = boot();
    accepted(&p.send(&mut host));
    p.op = 11;
    p.values = [12., 3., 1.5, 0., 0., 0.];
    let mut previous = None;
    for _ in 0..128 {
        let reply = p.send(&mut host);
        accepted(&reply);
        if let Some(old) = previous {
            assert!(!host.has_hold(old));
        }
        previous = Some(quoted(&reply, "translateToken"));
        assert_eq!(host.transform_drags.pair_count(), 1);
    }
    let reply = p.send(&mut host);
    accepted(&reply);
    let t = quoted(&reply, "translateToken");
    let s = quoted(&reply, "scaleToken");
    host.hold_end(t, HoldEnd::Cancel, 0.);
    assert_eq!(host.transform_drags.pair_count(), 1);
    assert!(host.has_hold(s));
    host.hold_end(s, HoldEnd::Cancel, 0.);
    assert_eq!(host.transform_drags.pair_count(), 0);
}

#[test]
fn suspended_ancestor_binding_returns_on_settle_without_an_app_receipt() {
    let source = SOURCE.replace(
        "    column\n",
        "    column testId=\"root\" transition=\"translate 100ms linear\"\n",
    );
    let (mut host, mut p, _) = boot_source(&source);
    accepted(&p.send(&mut host));
    let root = id(&host, "root");
    let begin = host.hold_begin(root, Property::Translate, 0.);
    assert!(begin.contains("\"op\":\"transform-drag\""), "{begin}");
    assert!(begin.contains("\"target\":null"), "{begin}");
    let token = quoted(&begin, "token");
    host.hold_update(token, MotionValue::new(20., 0.), 0.);
    host.hold_end(token, HoldEnd::Cancel, 0.);
    let early = host.tick(50.);
    assert!(!early.contains("\"op\":\"transform-drag\""), "{early}");
    let settled = host.tick(101.);
    assert!(settled.contains("\"op\":\"transform-drag\""), "{settled}");
    assert_eq!(quoted(&settled, "targetKey"), p.identity[2]);
    // Re-publication demands a new geometry observation, never stale Ready.
    p.op = 11;
    p.now = 101.;
    p.values = [0., 0., 1., 0., 0., 0.];
    assert!(p.send(&mut host).contains("\"accepted\":false"));
    p.op = 10;
    p.identity[4] += 1;
    p.values = [320.25, 200.5, 320.25, 200.5, 0., 0.];
    accepted(&p.send(&mut host));
    p.op = 11;
    p.values = [0., 0., 1., 0., 0., 0.];
    accepted(&p.send(&mut host));
}

/// A drag's release and its handle's geometry arrive from the gesture, not
/// through `dispatch_at`, after the app sat idle: nothing moved the runner's
/// clock since boot. Their actions run at the input's time, so `performanceNow()` is
/// that time and an `after` the release arms counts from it, not from the
/// clock's last value (it fired at the very next advance: a flung photo's
/// viewer closed the frame after the fling, Bluesky clone b09).
const TIMED: &str = r#"component App
  state away = false
  state landed = 0
  state releasedAt = -1
  state measuredAt = -1
  task fly when away
    after(300, land)
  action land
    landed = landed + 1
  action geometry(w: number, h: number, pw: number, ph: number)
    measuredAt = performanceNow()
  action finish(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    releasedAt = performanceNow()
    away = true
  view
    column testId="clip" width=320.25 height=200.5 overflow="hidden" padding=0 border-width=0
      column id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 transition="translate -exact-spring(180, 12, 1), scale -exact-spring(180, 12, 1)"
        column testId="handle" transformDragFor="photo" transformgeometry=geometry transformrelease=finish
"#;

#[test]
fn release_and_geometry_actions_run_at_the_inputs_time_after_an_idle_clock() {
    let (mut host, mut p, _) = boot_source(TIMED);
    p.now = 4000.0;
    accepted(&p.send(&mut host));
    assert_eq!(
        count(&host, "measuredAt"),
        4000.0,
        "the geometry action's performanceNow()"
    );
    p.op = 11;
    p.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 5000.0;
    let reply = p.send(&mut host);
    accepted(&reply);
    p.tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 13;
    p.values = [0.0, 120.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 5010.0;
    let reply = p.send(&mut host);
    accepted(&reply);
    assert!(reply.contains("\"committed\":true"), "{reply}");
    assert_eq!(
        count(&host, "releasedAt"),
        5010.0,
        "the release action's performanceNow()"
    );
    for token in p.tokens {
        host.hold_end(
            token,
            HoldEnd::Release {
                velocity: MotionValue::ZERO,
            },
            5010.0,
        );
    }
    host.advance(5100.0);
    assert_eq!(
        count(&host, "landed"),
        0.0,
        "armed at 5010, not at the clock's 4000"
    );
    host.advance(5309.0);
    assert_eq!(count(&host, "landed"), 0.0);
    host.advance(5310.0);
    assert_eq!(
        count(&host, "landed"),
        1.0,
        "due exactly 300 ms after the release"
    );
}

/// Timers due by a gesture's own event fire before it, and what they leave
/// is what the event meets: a timer that unbinds the photo ends the gesture
/// (the release action does not run, the pair is released as refused), and
/// one that drops the transition before new geometry makes the cancellation
/// snap rather than spring.
const RACED: &str = r#"component App
  state reference = "photo"
  state curve = "translate -exact-spring(180, 12, 1), scale -exact-spring(180, 12, 1)"
  state released = 0
  state measured = 0
  state unbindArmed = false
  state dropArmed = false
  task unbindLater when unbindArmed
    after(100, unbind)
  task dropLater when dropArmed
    after(100, drop)
  action unbind
    reference = "absent"
  action drop
    curve = "none"
  action armUnbind
    unbindArmed = true
  action armDrop
    dropArmed = true
  action geometry(w: number, h: number, pw: number, ph: number)
    measured = measured + 1
  action finish(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    released = released + 1
  view
    column
      button testId="arm-unbind" press=armUnbind width=80 height=24
      button testId="arm-drop" press=armDrop width=80 height=24
      column testId="clip" width=320.25 height=200.5 overflow="hidden" padding=0 border-width=0
        column id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 transition=curve
          column testId="handle" transformDragFor=reference transformgeometry=geometry transformrelease=finish
"#;

fn raced_hold(arm: &str) -> (Host<NoData>, Packet) {
    let (mut host, mut p, _) = boot_source(RACED);
    accepted(&p.send(&mut host));
    press(&mut host, arm, 1000.0);
    p.op = 11;
    p.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 1050.0;
    let reply = p.send(&mut host);
    accepted(&reply);
    p.tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 12;
    p.values = [0.0, 40.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 1060.0;
    accepted(&p.send(&mut host));
    (host, p)
}

#[test]
fn a_timer_that_unbinds_the_photo_before_its_release_ends_the_gesture() {
    let (mut host, mut p) = raced_hold("arm-unbind");
    // The unbind is due at 1100; the release comes at 1200, with nothing
    // having advanced the runner since the press.
    p.op = 13;
    p.values = [0.0, 60.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 1200.0;
    let reply = p.send(&mut host);
    assert!(reply.contains("\"committed\":false"), "{reply}");
    assert!(
        reply.contains("binding changed before its event"),
        "{reply}"
    );
    assert_eq!(
        count(&host, "released"),
        0.0,
        "the release action never ran"
    );
    assert_eq!(
        host.runner().slot("reference"),
        Some(&Value::str("absent")),
        "the timer's commit is kept"
    );
}

#[test]
fn a_timer_that_drops_the_transition_before_new_geometry_snaps_the_cancel() {
    let (mut host, mut p) = raced_hold("arm-drop");
    let photo = p.identity[2];
    p.op = 10;
    p.values = [300.0, 200.5, 300.0, 200.5, 0.0, 0.0];
    p.identity[4] = 2;
    p.tokens = [0; 2];
    p.now = 1200.0;
    let reply = p.send(&mut host);
    accepted(&reply);
    assert!(reply.contains("\"committed\":true"), "{reply}");
    assert_eq!(count(&host, "measured"), 2.0);
    assert!(
        !host.engine().is_active(photo, Property::Translate),
        "the cancel met the timer's `transition: none` and snapped"
    );
}

/// A timer due by the release that refuses (its action writes a number no
/// row takes) is reported, and the release still runs and commits; and new
/// geometry that changes nothing still commits the timers due by then.
const REFUSING: &str = r#"component App
  state n = 0
  state released = 0
  state armed = false
  state ticked = 0
  task bad when armed
    after(100, explode)
  task tick when armed
    after(50, count)
  action explode
    n = 1 / 0
  action count
    ticked = ticked + 1
  action arm
    armed = true
  action geometry(w: number, h: number, pw: number, ph: number)
    n = n
  action finish(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    released = released + 1
  view
    column
      button testId="arm" press=arm width=80 height=24
      column testId="clip" width=320.25 height=200.5 overflow="hidden" padding=0 border-width=0
        column id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 transition="translate -exact-spring(180, 12, 1)"
          column testId="handle" transformDragFor="photo" transformgeometry=geometry transformrelease=finish
"#;

#[test]
fn a_refusing_timer_is_reported_and_the_release_still_commits() {
    let (mut host, mut p, _) = boot_source(REFUSING);
    accepted(&p.send(&mut host));
    press(&mut host, "arm", 1000.0);
    // Unchanged geometry at 1060: the 50 ms timer is due and commits.
    p.now = 1060.0;
    p.identity[4] = 2;
    let reply = p.send(&mut host);
    assert!(reply.contains("\"accepted\":true"), "{reply}");
    assert_eq!(count(&host, "ticked"), 1.0, "{reply}");
    p.op = 11;
    p.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 1070.0;
    let reply = p.send(&mut host);
    accepted(&reply);
    p.tokens = [
        quoted(&reply, "translateToken"),
        quoted(&reply, "scaleToken"),
    ];
    p.op = 13;
    p.values = [0.0, 30.0, 1.0, 0.0, 0.0, 0.0];
    p.now = 1200.0;
    let reply = p.send(&mut host);
    assert!(reply.contains("\"committed\":true"), "{reply}");
    assert!(
        !reply.contains("\"error\":null"),
        "the timer's refusal is reported: {reply}"
    );
    assert_eq!(count(&host, "released"), 1.0);
}
