use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn lay_out(r: &mut Runner<NoData>) {
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
}

#[test]
fn native_intersect_mount_scroll_margin_clip_and_remount() {
    let plan = contract::compile(r#"component App
  state mounted = true
  state rooted = true
  action seen(e: IntersectionObserverEntry)
    mounted = mounted
  action unroot
    rooted = false
  action remove
    mounted = false
  action mount
    mounted = true
  view
    column width=200 height=100 overflow=(rooted ? "auto" : "visible") id="root"
      box height=150 flex-shrink=0
      if mounted
        box width=100 height=50 flex-shrink=0 id="target" intersect=seen intersect-margin="10% 0px 20px"
      box height=300 flex-shrink=0
"#).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    lay_out(&mut r);
    let view = |r: &Runner<NoData>, name: &str| {
        r.kernel()
            .arena()
            .local_id(r.kernel().find_by_id(name)[0].index)
    };
    let root = view(&r, "root");
    let target = view(&r, "target");
    let take = |r: &mut Runner<NoData>| {
        let due = r.intersect_due();
        for entry in &due {
            r.intersect_delivered(entry);
        }
        due.into_iter()
            .map(|e| (e.view, e.is_intersecting, e.intersection_ratio))
            .collect::<Vec<_>>()
    };
    let original = r.intersect_due()[0].clone();
    assert_eq!(take(&mut r), [(target, false, 0.0)]);
    assert!(take(&mut r).is_empty());
    r.scrolled(Some(root), 0.0, 30.0);
    assert_eq!(take(&mut r), [(target, true, 0.0)], "edge contact");
    r.scrolled(Some(root), 0.0, 60.0);
    assert!(take(&mut r).is_empty(), "ratio alone does not deliver");
    r.scrolled(Some(root), 0.0, 211.0);
    assert_eq!(
        take(&mut r),
        [(target, false, 0.0)],
        "vertical percent uses root height, as Chrome does"
    );
    r.act("remove", vec![]).unwrap();
    lay_out(&mut r);
    assert!(take(&mut r).is_empty(), "no synthetic leave on removal");
    r.act("mount", vec![]).unwrap();
    lay_out(&mut r);
    assert!(
        !r.intersect_delivered(&original),
        "queued entry cannot reach a recycled slot"
    );
    assert_eq!(take(&mut r).len(), 1, "a recycled slot gets a mount");
    r.scrolled(Some(root), 0.0, 60.0);
    assert!(take(&mut r)[0].1);
    r.act("unroot", vec![]).unwrap();
    lay_out(&mut r);
    assert_eq!(
        take(&mut r).len(),
        1,
        "changing root starts a new observation"
    );
}

#[test]
fn native_intersect_viewport_transforms_clips_and_zero_area() {
    let cases = [
        ("    scroll width=100 height=100\n      box height=150\n      box width=100 height=50 intersect=seen", false, 0.0),
        ("    scroll width=100 height=100 overflow-y=\"visible\"\n      box height=150\n      box width=100 height=50 intersect=seen", true, 1.0),
        ("    column\n      box width=100 height=50 translate=\"350px 0\" intersect=seen", true, 0.5),
        ("    column\n      box width=100 height=50 translate=\"400px 0\" intersect=seen", true, 0.0),
        ("    column\n      box width=100 height=50 translate=\"401px 0\" intersect=seen", false, 0.0),
        ("    column\n      box width=0 height=0 intersect=seen", true, 1.0),
        ("    column display=\"none\"\n      box width=100 height=50 intersect=seen", false, 0.0),
        ("    column width=100 height=100 overflow=\"hidden\" translate=\"1000px 0\"\n      box width=100 height=50 intersect=seen", true, 1.0),
        ("    column width=100 height=100 overflow=\"hidden\"\n      column clip-path=\"path('M 0 0 L 30 0 L 30 100 L 0 100 Z')\"\n        box width=100 height=50 translate=\"20px 0\" intersect=seen intersect-margin=\"200px\"", true, 0.1),
        ("    column width=200 height=200 translate=\"100px 100px\" rotate=45\n      box width=200 height=200 rotate=-45 intersect=seen", true, 1.0),
    ];
    for (view, inside, ratio) in cases {
        let source = format!("component App\n  state count = 0\n  action seen(e: IntersectionObserverEntry)\n    count = count + 1\n  view\n{view}\n");
        let plan = contract::compile(&source).unwrap();
        let mut r = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        lay_out(&mut r);
        let entries = r.intersect_due();
        assert_eq!(entries.len(), 1, "{view}");
        assert_eq!(entries[0].is_intersecting, inside, "{view}");
        assert!(
            (entries[0].intersection_ratio - ratio).abs() < 0.00001,
            "{view}: {} != {ratio}",
            entries[0].intersection_ratio
        );
    }
}

#[test]
fn native_intersect_margin_units_are_case_insensitive() {
    for margin in ["200PX", "200Px", "0px 0PX 200pX 0px"] {
        let source = format!(
            r#"component App
  state seenValue = false
  action seen(e: IntersectionObserverEntry)
    seenValue = e.isIntersecting
  view
    scroll width=200 height=100
      box height=200
      box width=100 height=50 intersect=seen intersect-margin="{margin}"
"#
        );
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        lay_out(&mut r);
        let entries = r.intersect_due();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_intersecting, "{margin}");
        assert_eq!(entries[0].intersection_ratio, 1.0, "{margin}");
    }
}

#[test]
fn native_intersect_entries_follow_registration_order() {
    let mut source = String::from("component App\n  state seenValue = false\n  action seen(e: IntersectionObserverEntry)\n    seenValue = e.isIntersecting\n  view\n    scroll width=200 height=100\n");
    for i in 0..32 {
        source.push_str(&format!(
            "      box id=\"row-{i}\" height=10 intersect=seen\n"
        ));
    }
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    lay_out(&mut r);
    let expected: Vec<_> = (0..32)
        .map(|i| {
            r.kernel()
                .arena()
                .local_id(r.kernel().find_by_id(&format!("row-{i}"))[0].index)
        })
        .collect();
    let due = r.intersect_due();
    assert_eq!(due.iter().map(|e| e.view).collect::<Vec<_>>(), expected);
    for entry in due {
        assert!(r.intersect_delivered(&entry));
    }
    let root = r.kernel().node(expected[0]).unwrap().parent.unwrap();
    r.scrolled(Some(root), 0.0, 250.0);
    let entered: Vec<_> = r.intersect_due().iter().map(|e| e.view).collect();
    assert!(entered.windows(2).all(|w| w[0] < w[1]), "{entered:?}");
}

#[test]
fn intersect_margin_requires_css_number_syntax() {
    for margin in [
        "1.px", "1.%", ".px", "1e", "1epx", "1e+px", "+-1px", "1.e2px",
    ] {
        let error = contract::compile(&format!(
            "component App\n  view\n    box intersect-margin={margin:?}\n"
        ))
        .expect_err(margin);
        assert_eq!(error.id, "lower-intersect-margin", "{margin}: {error:?}");
    }
    for margin in [
        "+1px", "-.5%", "+.5PX", "1.0px", "1e2px", "1E-2%", ".5e+2px",
    ] {
        contract::compile(&format!(
            "component App\n  view\n    box intersect-margin={margin:?}\n"
        ))
        .unwrap_or_else(|error| panic!("{margin}: {error:?}"));
    }
}

#[test]
fn native_intersect_sticky_target_and_descendant_follow_scroll() {
    for target in [
        "        box position=\"sticky\" top=0 width=100 height=50 intersect=seen",
        "        box position=\"sticky\" top=0 width=100 height=50\n          box width=100 height=50 intersect=seen",
    ] {
        let source = format!(
            "component App\n  state count = 0\n  action seen(e: IntersectionObserverEntry)\n    count = count + 1\n  view\n    scroll width=100 height=100 id=\"root\"\n      box height=200\n{target}\n      box height=300\n"
        );
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        lay_out(&mut r);
        let root = r.kernel().arena().local_id(r.kernel().find_by_id("root")[0].index);
        r.scrolled(Some(root), 0.0, 60.0);
        let entries = r.intersect_due();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_intersecting, "{target}");
        assert_eq!(entries[0].intersection_ratio, 1.0);
        assert!(r.intersect_delivered(&entries[0]));
        r.scrolled(Some(root), 0.0, 150.0);
        assert!(r.intersect_due().is_empty(), "still pinned at the section's end");
        r.scrolled(Some(root), 0.0, 201.0);
        let entries = r.intersect_due();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].is_intersecting, "left its containing section");
    }
}

#[test]
fn native_intersect_sticky_page_scroll_uses_root_local_coordinates() {
    let source = r#"component App
  state count = 0
  action seen(e: IntersectionObserverEntry)
    count = count + 1
  view
    box margin-top=500 height=1000
      box position="sticky" top=0 width=100 height=50 intersect=seen
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        exact_runner::Viewport {
            width: 400.0,
            height: 400.0,
            ..Default::default()
        },
        "/",
    )
    .unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 400.0))
        .unwrap();
    assert_eq!(r.kernel().arena().frame(root).y, 500.0);
    let entries = r.intersect_due();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].is_intersecting);
    assert!(r.intersect_delivered(&entries[0]));
    r.scrolled(None, 0.0, 450.0);
    let entries = r.intersect_due();
    assert_eq!(entries.len(), 1, "the target enters at viewport y=50");
    assert!(entries[0].is_intersecting);
    assert_eq!(entries[0].intersection_ratio, 1.0);
}

#[test]
fn native_intersect_rotated_root_ratios_match_chrome() {
    for (root_style, target_style, expected) in [
        (
            "rotate=45",
            "width=100 height=50 translate=\"0 80px\"",
            0.64,
        ),
        ("rotate=45", "width=100 height=100 rotate=-45", 1.0),
        (
            "rotate=30 scale=2",
            "width=100 height=50 translate=\"0 80px\"",
            0.6245602369308472,
        ),
    ] {
        let source = format!(
            "component App\n  state count = 0\n  action seen(e: IntersectionObserverEntry)\n    count = count + 1\n  view\n    box width=100 height=100 overflow=\"hidden\" {root_style}\n      box {target_style} intersect=seen\n"
        );
        let mut r = Runner::boot(
            contract::compile(&source).unwrap(),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        lay_out(&mut r);
        let entries = r.intersect_due();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_intersecting);
        assert!(
            (entries[0].intersection_ratio - expected).abs() < 0.000001,
            "{root_style}: {entries:?}"
        );
    }
}
