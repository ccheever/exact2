//! Real Contract photo lifetime, contained-image bounds and paired held targets.
//! Physical pointer generations and end delivery remain shared-host tests.
use exact_kernel::{motion_node, CommitReceipt, Kernel, NodeKey, NodeType, Offer, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};
use interaction_gallery_data::Gallery;
use std::{path::Path, rc::Rc};

#[derive(Default)]
struct CountedGallery {
    inner: Gallery,
    calls: usize,
}

impl DataSource for CountedGallery {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls += 1;
        self.inner.query(source, args)
    }
}

fn boot() -> Runner<CountedGallery> {
    Runner::boot(
        contract::compile(include_str!("../../../app.contract")).unwrap(),
        CountedGallery::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn key(r: &Runner<CountedGallery>, name: &str) -> NodeKey {
    let keys = r.kernel().find_by_test_id(name);
    assert_eq!(keys.len(), 1, "missing or ambiguous {name}");
    keys[0]
}

fn event(r: &mut Runner<CountedGallery>, name: &str, event: Event) -> CommitReceipt {
    let id = r.kernel().node_by_key(key(r, name)).unwrap().id;
    r.dispatch(id, event).unwrap()
}

fn press(r: &mut Runner<CountedGallery>, name: &str) -> CommitReceipt {
    event(r, name, Event::Press)
}

fn command(r: &mut Runner<CountedGallery>, op: &str, id: &str, n: f64) -> CommitReceipt {
    r.act(
        "command",
        vec![Value::str(op), Value::str(id), Value::Number(n)],
    )
    .unwrap()
}

fn geometry(r: &mut Runner<CountedGallery>, width: f64, height: f64) -> CommitReceipt {
    event(
        r,
        "viewer-handle",
        Event::TransformGeometry {
            box_width: width,
            box_height: height,
            port_width: width,
            port_height: height,
        },
    )
}

fn release(r: &mut Runner<CountedGallery>, x: f64, y: f64, scale: f64) -> CommitReceipt {
    event(
        r,
        "viewer-handle",
        Event::TransformRelease {
            x,
            y,
            scale,
            vx: 120.,
            vy: -90.,
            vscale: 0.,
        },
    )
}

fn targets(r: &Runner<CountedGallery>) -> [f64; 3] {
    let style = r
        .kernel()
        .node_by_key(key(r, "viewer-transform"))
        .unwrap()
        .style;
    [
        style.translate.x as f64,
        style.translate.y as f64,
        style.scale as f64,
    ]
}

fn close_to(actual: [f64; 3], expected: [f64; 3]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
    }
}

fn rows(r: &Runner<CountedGallery>) -> Rc<[Value]> {
    let Value::List(rows) = r.resource("rows").unwrap() else {
        panic!("row resource")
    };
    rows.clone()
}

#[test]
fn authored_binding_resolves_full_size_target_and_actual_direct_clip() {
    let mut r = boot();
    press(&mut r, "open-photo-00000");
    let handle = key(&r, "viewer-handle");
    let target = key(&r, "viewer-transform");
    let clip = key(&r, "viewer-clip");
    let binding = r.kernel().transform_drag_binding(handle).unwrap();
    assert_eq!(
        (binding.handle, binding.target, binding.clip),
        (handle, target, clip)
    );
    assert_eq!(
        r.kernel().node_by_key(handle).unwrap().node_type,
        NodeType::View
    );
    assert_eq!(
        r.kernel()
            .node_by_key(handle)
            .unwrap()
            .props
            .str(PropId::TransformDragFor),
        Some("viewer-transform")
    );
    assert_eq!(
        r.kernel()
            .node_by_key(target)
            .unwrap()
            .props
            .str(PropId::Id),
        Some("viewer-transform")
    );
    for control in [
        "viewer-zoom-fit",
        "viewer-zoom-detail",
        "viewer-reset",
        "close-viewer",
        "next-photo",
    ] {
        assert!(r
            .kernel()
            .transform_drag_binding(key(&r, control))
            .is_none());
    }
    for width in [1180., 420.] {
        let root = r.roots()[0];
        r.kernel_mut()
            .compute_layout(root, Offer::definite(width, 860.))
            .unwrap();
        let target_frame = r.kernel().node_by_key(target).unwrap().frame;
        let clip_frame = r.kernel().node_by_key(clip).unwrap().frame;
        assert!(target_frame.width > 0. && target_frame.height > 0.);
        assert_eq!(target_frame.width, clip_frame.width);
        assert_eq!(target_frame.height, clip_frame.height);
        geometry(&mut r, clip_frame.width as f64, clip_frame.height as f64);
        press(&mut r, "viewer-zoom-detail");
        release(&mut r, 9999., -9999., 2.);
        let width = clip_frame.width as f64;
        let height = clip_frame.height as f64;
        let fit = (width / 1448.).min(height / 1086.);
        close_to(
            targets(&r),
            [
                ((1448. * fit * 2. - width) / 2.).max(0.),
                -((1086. * fit * 2. - height) / 2.).max(0.),
                2.,
            ],
        );
    }
}

#[test]
fn contain_bounds_cover_letterboxing_signed_edges_and_incoming_resize_values() {
    let mut r = boot();
    press(&mut r, "open-photo-00000");
    let source = rows(&r);
    let calls = r.data_ref().calls;
    geometry(&mut r, 800., 400.);
    press(&mut r, "viewer-zoom-detail");
    for (x, y, expected) in [
        (1000., -1000., [400. / 3., -200., 2.]),
        (-1000., 1000., [-400. / 3., 200., 2.]),
        (100., -150., [100., -150., 2.]),
        (400. / 3., 200., [400. / 3., 200., 2.]),
    ] {
        release(&mut r, x, y, 2.);
        close_to(targets(&r), expected);
    }
    // The action must clamp using its NEW dimensions, not pre-action slot reads.
    geometry(&mut r, 300., 600.);
    close_to(targets(&r), [400. / 3., 0., 2.]);
    release(&mut r, -1000., 1000., 2.);
    close_to(targets(&r), [-150., 0., 2.]);
    // Choosing fit must use the NEW zoom argument in the same transaction.
    press(&mut r, "viewer-zoom-fit");
    close_to(targets(&r), [0., 0., 1.]);
    press(&mut r, "viewer-zoom-detail");
    release(&mut r, 100., 20., 2.);
    press(&mut r, "viewer-reset");
    close_to(targets(&r), [0., 0., 1.]);
    assert_eq!(r.data_ref().calls, calls, "no per-gesture data actions");
    assert!(Rc::ptr_eq(&source, &rows(&r)));
}

#[test]
fn zero_geometry_is_neutral_and_equal_geometry_does_not_touch_target() {
    let mut r = boot();
    press(&mut r, "open-photo-00000");
    press(&mut r, "viewer-zoom-detail");
    release(&mut r, 999., -999., 2.);
    close_to(targets(&r), [0., 0., 2.]);
    geometry(&mut r, 800., 400.);
    release(&mut r, 100., -100., 2.);
    let target = key(&r, "viewer-transform");
    let same = geometry(&mut r, 800., 400.);
    assert!(!same.touched.contains(&target));
    close_to(targets(&r), [100., -100., 2.]);
    geometry(&mut r, 0., 400.);
    close_to(targets(&r), [0., 0., 2.]);
    release(&mut r, 999., 999., 2.);
    close_to(targets(&r), [0., 0., 2.]);
    geometry(&mut r, 800., 0.);
    close_to(targets(&r), [0., 0., 2.]);
}

#[test]
fn fresh_metadata_rank_and_typing_keep_child_state_but_new_source_resets_it() {
    let mut r = boot();
    press(&mut r, "open-photo-00000");
    geometry(&mut r, 800., 400.);
    press(&mut r, "viewer-zoom-detail");
    release(&mut r, 100., -120., 2.);
    let original = key(&r, "viewer-transform");
    r.act("edit", vec![Value::str("retained draft")]).unwrap();
    for (op, id, n) in [
        ("select", "photo-00000", 0.),
        ("page", "", 3.),
        ("insert", "", 0.),
    ] {
        command(&mut r, op, id, n);
        assert_eq!(key(&r, "viewer-transform"), original);
        close_to(targets(&r), [100., -120., 2.]);
    }
    command(&mut r, "select", "photo-00001", 0.);
    assert!(r.kernel().node_by_key(original).is_none());
    assert_ne!(key(&r, "viewer-transform"), original);
    close_to(targets(&r), [0., 0., 1.]);
    // The fresh child has no inherited geometry even before host feedback.
    press(&mut r, "viewer-zoom-detail");
    release(&mut r, 200., 200., 2.);
    close_to(targets(&r), [0., 0., 2.]);
    assert_eq!(r.slot("draft"), Some(&Value::str("retained draft")));
}

#[test]
fn navigation_reset_delete_and_lift_end_the_keyed_viewer_lifetime() {
    for action in [
        "next-photo",
        "close-viewer",
        "reset",
        "delete-photo",
        "lift",
    ] {
        let mut r = boot();
        press(&mut r, "open-photo-00000");
        geometry(&mut r, 800., 400.);
        press(&mut r, "viewer-zoom-detail");
        release(&mut r, 100., 100., 2.);
        let old = key(&r, "viewer-transform");
        let old_handle = key(&r, "viewer-handle");
        if action == "lift" {
            command(&mut r, "lift", "photo-00001", 0.);
        } else {
            press(&mut r, action);
        }
        assert!(r.kernel().node_by_key(old).is_none(), "{action}");
        assert!(r.kernel().node_by_key(old_handle).is_none(), "{action}");
        assert!(r.kernel().transform_drag_binding(old_handle).is_none());
        if action == "next-photo" {
            close_to(targets(&r), [0., 0., 1.]);
        } else {
            assert!(r.kernel().find_by_test_id("viewer-transform").is_empty());
            command(&mut r, "open", "photo-00002", 0.);
            close_to(targets(&r), [0., 0., 1.]);
        }
    }
}

#[test]
fn latest_authored_zoom_and_pan_land_while_both_real_engine_holds_remain_live() {
    let mut r = boot();
    let opened = press(&mut r, "open-photo-00000");
    let owner = key(&r, "viewer-transform");
    let node = motion_node(owner);
    // Infer the existing Engine/Value types through MotionSync; no app motion dependency.
    let mut engine = Default::default();
    r.kernel().motion_sync(&opened).apply(&mut engine).unwrap();
    geometry(&mut r, 800., 400.);
    let style = r.kernel().node_by_key(owner).unwrap().style;
    let mut translate = exact_kernel::motion::targets(style)[0].1;
    let mut scale = exact_kernel::motion::targets(style)[1].1;
    translate.x = 250.;
    translate.y = -300.;
    scale.x = 1.3;
    let pair = engine
        .begin_transform_hold(node, 0., Some([translate, scale]))
        .unwrap()
        .unwrap();
    let calls = r.data_ref().calls;
    for (control, expected) in [
        ("viewer-zoom-detail", [400. / 3., -200., 2.]),
        ("viewer-zoom-fit", [0., 0., 1.]),
    ] {
        let chosen = press(&mut r, control);
        r.kernel().motion_sync(&chosen).apply(&mut engine).unwrap();
        let released = release(&mut r, 1000., -1000., expected[2]);
        r.kernel()
            .motion_sync(&released)
            .apply(&mut engine)
            .unwrap();
        close_to(targets(&r), expected);
        for (start, held) in [(pair.translate(), translate), (pair.scale(), scale)] {
            assert!(engine.has_hold(start.token));
            assert_eq!(engine.value(node, start.token.property()), Some(held));
        }
        let pan = engine
            .target(node, pair.translate().token.property())
            .unwrap();
        let zoom = engine.target(node, pair.scale().token.property()).unwrap();
        close_to([pan.x, pan.y, zoom.x], expected);
        assert!(engine.quiescent());
        assert_eq!(r.data_ref().calls, calls);
    }
    // The app proves the pre-end target transaction. Individual end/velocity
    // delivery is covered by exact-motion and the shared host adapters.
    let replacement = press(&mut r, "next-photo");
    let sync = r.kernel().motion_sync(&replacement);
    assert!(sync.removed.contains(&node));
    sync.apply(&mut engine).unwrap();
    assert!(!engine.has_hold(pair.translate().token));
    assert!(!engine.has_hold(pair.scale().token));
    assert!(!engine
        .update_transform_hold(pair, f64::NAN, [translate, scale])
        .unwrap());
    assert_ne!(key(&r, "viewer-transform"), owner);
    close_to(targets(&r), [0., 0., 1.]);
}

#[test]
fn viewer_dimensions_match_each_immutable_png_and_its_provenance_record() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let manifest = std::fs::read_to_string(root.join("assets/provenance.json")).unwrap();
    let mut data = Gallery::default();
    for index in 0..6 {
        let value = data
            .query(
                "galleryAction",
                &[
                    Value::str("open"),
                    Value::str(&format!("photo-{index:05}")),
                    Value::Number(0.),
                ],
            )
            .unwrap();
        let Value::Record(metadata) = value else {
            panic!("metadata")
        };
        let Value::List(viewer) = &metadata[25] else {
            panic!("viewer list")
        };
        let Value::Record(viewer) = &viewer[0] else {
            panic!("viewer")
        };
        let Value::Record(photo) = &viewer[1] else {
            panic!("photo")
        };
        assert_eq!(photo.len(), 8);
        let asset = photo[3].as_str().unwrap();
        let bytes = std::fs::read(root.join(asset)).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&bytes[12..16], b"IHDR");
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        assert_eq!((width, height), (1448, 1086));
        assert_eq!(viewer[2], Value::Number(width as f64));
        assert_eq!(viewer[3], Value::Number(height as f64));
        let filename = asset.strip_prefix("assets/").unwrap();
        let entry = manifest
            .split("\"file\":")
            .find(|entry| entry.trim_start().starts_with(&format!("\"{filename}\"")))
            .unwrap();
        let entry = entry.split('}').next().unwrap();
        assert!(entry.contains("\"width\": 1448"));
        assert!(entry.contains("\"height\": 1086"));
    }
}

/// LLP 1057.001 §4: a pinch's release adopts its scale, within Fit and 4×,
/// and pan is clamped against that new zoom in the same transaction.
#[test]
fn a_pinch_release_adopts_its_scale_within_fit_and_four_times() {
    let mut r = boot();
    press(&mut r, "open-photo-00000");
    geometry(&mut r, 800., 400.);
    let fit = (800f64 / 1448.).min(400. / 1086.);
    let bound = |side: f64, port: f64, zoom: f64| ((side * fit * zoom - port) / 2.).max(0.);
    release(&mut r, 9999., -9999., 3.);
    close_to(
        targets(&r),
        [bound(1448., 800., 3.), -bound(1086., 400., 3.), 3.],
    );
    release(&mut r, 0., 0., 9.);
    close_to(targets(&r), [0., 0., 4.]);
    release(&mut r, 9999., 9999., 0.5);
    close_to(targets(&r), [0., 0., 1.]);
    press(&mut r, "viewer-zoom-detail");
    close_to(targets(&r), [0., 0., 2.]);
}
