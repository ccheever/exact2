//! Actual Web binary boundary: generations, whole-pair admission and receipt order.
use exact_motion::{HoldEnd, Property, Value as MotionValue};
use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::{abi::Bridge, Host};

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
          column id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 translate=`${x}px ${y}px` scale=zoom transition="translate spring(180, 12, 1), scale spring(180, 12, 1)"
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
    exact_web::link(exact_web_capabilities::ALL);
    let (host, batch) = Host::boot(plan, NoData, Default::default(), "/").unwrap();
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
        .map(|property| host.springs().engine().value(node, property).unwrap())
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
    assert_eq!(host.springs().engine().now(), 0.0);
}

#[test]
fn full_live_payload_preflight_preserves_both_samples_tokens_and_clock() {
    let (mut host, p) = held();
    let before = current(&host, p.identity[2]);
    let time = host.springs().engine().now();
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
            assert_eq!(host.springs().engine().now(), time);
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
        assert_eq!(host.springs().engine().now(), time);
    }
    for now in [-1.0, 99.0, f64::INFINITY] {
        assert!(Packet { op: 13, now, ..p }
            .send(&mut host)
            .contains("\"error\":\""));
        assert_eq!(current(&host, p.identity[2]), before);
        assert_eq!(host.springs().engine().now(), time);
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
        assert_eq!(host.springs().engine().now(), 0.1);
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
        assert_eq!(host.springs().engine().now(), 0.1);
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
        .replace("transition=\"translate spring(180, 12, 1), scale spring(180, 12, 1)\"", "transition=(bw > 400 ? \"none\" : \"translate spring(180, 12, 1), scale spring(180, 12, 1)\")");
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
    assert_eq!(host.springs().engine().now(), 0.2);
    assert!(tokens.into_iter().all(|token| !host.has_hold(token)));
    for prop in [Property::Translate, Property::Scale] {
        assert!(host
            .springs()
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
    // LLP 1057.001 §3: begin at 100 ms, terminal at 110 ms.
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
        assert!(host
            .end_hold(token, HoldEnd::Release { velocity }, 110.0)
            .unwrap()
            .is_some());
    }
    assert_eq!(count(&host, "released"), 1.0);
    assert_eq!(
        host.springs()
            .engine()
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
    let (replacement, _) = host
        .begin_hold(target, Property::Scale, MotionValue::scalar(1.8), 110.0)
        .unwrap()
        .unwrap();
    p.now = 1e9;
    p.values = [f64::NAN; 6];
    assert!(p.send(&mut host).contains("\"accepted\":false"));
    assert!(host.has_hold(replacement.token.serial()));
    assert!(!host.has_hold(p.tokens[0]));
    assert_eq!(host.springs().engine().now(), 0.11);
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
        reply.find("\"op\":\"retire-motion\"").unwrap() < reply.find("\"op\":\"animate\"").unwrap()
    );
    assert_eq!(
        host.springs()
            .engine()
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
    // The Web result must name failed kernel publication, not promise rollback.
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
        assert!(host
            .end_hold(token, HoldEnd::Cancel, 1e9)
            .unwrap()
            .is_none());
    }
    assert_eq!(host.springs().engine().now(), 0.1);
    assert!(p.send(&mut host).contains("\"accepted\":false"));
}

#[test]
fn wasm_bridge_preserves_runtime_keys_sequence_and_rejects_wrong_packet_length() {
    let mut bridge = Bridge::new();
    bridge.set_links(exact_web::HostLinks::ALL);
    exact_web::link(exact_web_capabilities::ALL);
    let n = bridge.boot(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        400.0,
        800.0,
        "/",
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
    let n = bridge.motion(bytes.len());
    accepted(std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap());
    for len in [0, 48, 119, 121] {
        bridge.input_write(&bytes);
        let n = bridge.motion(len);
        let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(out.contains("\"error\":\""), "{len}: {out}");
    }
    let mut begin = Packet {
        op: 11,
        values: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        ..p
    };
    bridge.input_write(&begin.bytes());
    let n = bridge.motion(120);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    accepted(out);
    assert_eq!(quoted(out, "geometrySequence"), 9007199254740993);
    begin.tokens = [quoted(out, "translateToken"), quoted(out, "scaleToken")];
    assert_ne!(begin.tokens[0], begin.tokens[1]);
}

#[test]
fn each_missing_handler_refuses_publication_geometry_and_pair_admission() {
    for missing in [" transformgeometry=geometry", " transformrelease=finish"] {
        let source = SOURCE.replace(missing, "");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-transform-drag-handlers");
        // Authored plans are refused by the compiler. A decoded plan must
        // still meet the host's handler-pair check at its binary boundary.
        let mut plan = contract::compile(SOURCE).unwrap();
        let event = if missing.contains("transformgeometry") {
            exact_plan::EventKind::Transformgeometry
        } else {
            exact_plan::EventKind::Transformrelease
        };
        let handler = plan.handlers.iter().position(|h| h.event == event).unwrap() as u32;
        let node = plan
            .nodes
            .iter_mut()
            .find(|n| n.handlers.iter().any(|h| h.0 == handler))
            .unwrap();
        assert_eq!(node.handlers.len, 2);
        if node.handlers.start == handler {
            node.handlers.start += 1;
        }
        node.handlers.len -= 1;
        exact_web::link(exact_web_capabilities::ALL);
        let (mut host, batch) =
            Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
        let publication = batch.split("\"op\":\"transform-drag\"").nth(1).unwrap();
        assert!(
            publication.contains("\"target\":null"),
            "missing {missing}: {publication}"
        );
        let handle = host.runner().kernel().find_by_test_id("handle")[0];
        // The kernel's source resolver deliberately does not own event handlers.
        let binding = host
            .runner()
            .kernel()
            .transform_drag_binding(handle)
            .unwrap();
        let mut p = Packet {
            op: 10,
            identity: [
                quoted(publication, "runtime"),
                exact_kernel::motion::motion_node(handle),
                exact_kernel::motion::motion_node(binding.target),
                exact_kernel::motion::motion_node(binding.clip),
                1,
            ],
            tokens: [0; 2],
            values: [320.25, 200.5, 320.25, 200.5, 0.0, 0.0],
            now: 100.0,
        };
        assert!(p.send(&mut host).contains("\"accepted\":false"));
        p.op = 11;
        p.values = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        assert!(p.send(&mut host).contains("\"accepted\":false"));
        assert_eq!(host.springs().engine().now(), 0.0);
        assert_eq!(count(&host, "measured"), 0.0);
    }
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
    assert_eq!(host.springs().engine().now(), 0.2);
    assert_eq!(count(&host, "released"), 0.0);
    for token in tokens {
        assert!(reply.contains(&format!("\"token\":\"{token}\"")), "{reply}");
    }
}

/// A drag's release and its handle's geometry arrive from the gesture, not
/// through `dispatch_at`, after the page sat idle: nothing moved the runner's
/// clock since boot. Their actions run at the input's time, so `now()` is
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
    measuredAt = now()
  action finish(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    releasedAt = now()
    away = true
  view
    column testId="clip" width=320.25 height=200.5 overflow="hidden" padding=0 border-width=0
      column id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 transition="translate spring(180, 12, 1), scale spring(180, 12, 1)"
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
        "the geometry action's now()"
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
        "the release action's now()"
    );
    for token in p.tokens {
        assert!(host
            .end_hold(
                token,
                HoldEnd::Release {
                    velocity: MotionValue::ZERO
                },
                5010.0
            )
            .unwrap()
            .is_some());
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
