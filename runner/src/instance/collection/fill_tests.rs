//! Limited reports (@ref LLP 1050.000 §6): owed rows, lead order, retirement.
use super::*;

#[test]
fn a_limited_report_builds_what_it_owes_then_leads_in_travel_order() {
    let mut h = Harness::new(1_000, false, false);
    let rows = |h: &Harness| {
        h.snapshot()
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
    };
    h.send(h.feedback(3200.0));
    let settled = rows(&h);
    assert!(!h.snapshot().pending);
    assert!(settled.contains(&90) && settled.contains(&119) && !settled.contains(&120));
    // Two viewports down at 1,600 px/s, with room for two rows past what shows.
    let down = CollectionFill {
        velocity: 1_600.0,
        limit: Some(2),
        ..Default::default()
    };
    h.send_filled(h.feedback(3840.0), down);
    let now = rows(&h);
    assert!(
        (120..130).all(|i| now.contains(&i)),
        "the visible rows are owed: {now:?}"
    );
    assert_eq!(
        now.iter()
            .filter(|&&i| i >= 130)
            .copied()
            .collect::<Vec<_>>(),
        [130, 131]
    );
    assert_eq!(
        settled
            .iter()
            .copied()
            .filter(|i| !now.contains(i))
            .collect::<Vec<_>>(),
        (90..99).collect::<Vec<_>>(),
        "rows two viewports past what shows retire at once, past the cap"
    );
    assert!(h.snapshot().pending);
    assert!(
        now.windows(2).all(|w| w[0] < w[1]),
        "rows stay in logical order"
    );
    // Continuations with the same facts finish the led window, then stop.
    let mut reports = 0;
    while h.snapshot().pending {
        let mut f = h.feedback(3840.0);
        f.scroll_sequence -= 1;
        h.send_filled(f, down);
        reports += 1;
        assert!(reports < 40);
    }
    let led = rows(&h);
    let last = *led.last().unwrap();
    // A quarter second of travel (400 px) past the viewport's overscan.
    assert!((152..=153).contains(&last), "{led:?}");
    assert!(*led.first().unwrap() >= 109, "{led:?}");
    // Upward travel leads the other way: rows above are built first.
    let up = CollectionFill {
        velocity: -1_600.0,
        limit: Some(1),
        ..Default::default()
    };
    h.send_filled(h.feedback(3200.0), up);
    let now = rows(&h);
    assert!((100..110).all(|i| now.contains(&i)));
    assert_eq!(now.first(), Some(&99), "{now:?}");
}

#[test]
fn travel_inside_the_realized_window_moves_only_the_geometry() {
    let mut h = Harness::new(1_000, false, false);
    // A host reports every mounted row's height, every time.
    let report = |h: &Harness, top: f64, grow: f64| {
        let mut f = h.feedback(top);
        let rows = h.snapshot().rows;
        f.measurements = rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: r.size + if r.index == rows[0].index { grow } else { 0.0 },
            })
            .collect();
        f
    };
    h.send(h.feedback(3216.0));
    h.send(report(&h, 3216.0, 0.0));
    let before = h.snapshot();
    let children = h.collection().children.clone();
    // Rows are 32 px: every top in (3200, 3232) leads to the same window.
    for top in [3217.0, 3220.5, 3231.0] {
        assert!(
            !h.send(h.feedback(top)),
            "no row, extent or correction changes"
        );
        assert!(
            !h.send(report(&h, top, 0.0)),
            "the same heights again change nothing"
        );
    }
    let after = h.snapshot();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.rows, before.rows);
    assert_eq!(h.collection().children, children);
    assert_eq!(h.collection().geometry.as_ref().unwrap().offset, 3231.0);
    // A translated row remeasures float32 ulps off (58 as 57.99997): noise,
    // not a measurement, so the revision a drop certified stays.
    assert!(!h.send(report(&h, 3231.0, -0.00003)));
    assert_eq!(h.snapshot().revision, before.revision);
    assert_eq!(h.snapshot().rows, before.rows);
    // A new height is a measurement: the window realizes again.
    assert!(h.send(report(&h, 3231.0, 8.0)));
    assert_ne!(h.snapshot().revision, before.revision);
    // Past the window, a report realizes as one sent there directly does.
    let mut fresh = Harness::new(1_000, false, false);
    fresh.send(fresh.feedback(3216.0));
    fresh.send(report(&fresh, 3216.0, 0.0));
    fresh.send(report(&fresh, 3231.0, 8.0));
    h.send(h.feedback(3400.0));
    fresh.send(fresh.feedback(3400.0));
    let rows = |s: CollectionSnapshot| {
        s.rows
            .iter()
            .map(|r| (r.index, r.start, r.size))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(h.snapshot()), rows(fresh.snapshot()));
}

#[test]
fn a_limited_report_retires_nearer_rows_at_its_cap() {
    let mut h = Harness::new(1_000, false, false);
    let rows = |h: &Harness| {
        h.snapshot()
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
    };
    h.send(h.feedback(3200.0));
    let settled = rows(&h);
    // One viewport down: rows 90..99 leave the window, none of them two
    // viewports away, so a report that may build two retires four.
    let down = CollectionFill {
        velocity: 1_600.0,
        limit: Some(2),
        ..Default::default()
    };
    h.send_filled(h.feedback(3520.0), down);
    let now = rows(&h);
    assert_eq!(
        settled.iter().filter(|i| !now.contains(i)).count(),
        4,
        "retirement is bounded"
    );
    assert!(
        !now.contains(&90) && now.contains(&94),
        "the farthest rows retire first"
    );
    assert!(h.snapshot().pending);
}

#[test]
fn rows_kept_past_the_window_never_outnumber_the_window() {
    let mut h = Harness::new(5_000, false, false);
    h.send(h.feedback(3200.0));
    let settled = h.snapshot().rows.len();
    // Travel that outruns the fill: every report is a rescue (limit 0),
    // and each lands a viewport and more past the last.
    let rescue = CollectionFill {
        velocity: 48_000.0,
        limit: Some(0),
        ..Default::default()
    };
    for step in 1..=40 {
        let top = 3200.0 + 400.0 * step as f64;
        h.send_filled(h.feedback(top), rescue);
        let rows: Vec<usize> = h.snapshot().rows.iter().map(|r| r.index).collect();
        let first = (top / 32.0) as usize;
        assert!(
            (first..first + 10).all(|i| rows.contains(&i)),
            "the visible rows are owed: {rows:?}"
        );
        assert!(
            rows.len() <= 2 * settled,
            "report {step}: {} rows mounted, the window settled at {settled}",
            rows.len()
        );
    }
    // At rest an unlimited report retires everything past the window.
    h.send(h.feedback(3200.0 + 400.0 * 40.0));
    assert_eq!(h.snapshot().rows.len(), settled);
}

/// @ref LLP 1072 §5 — a build-only report then a retire-only one leave the
/// rows one immediate report leaves, and the build-only one destroys none.
#[test]
fn a_build_only_report_then_a_retire_only_one_equal_one_immediate_report() {
    let rows = |h: &Harness| {
        h.snapshot()
            .rows
            .iter()
            .map(|r| r.index)
            .collect::<Vec<_>>()
    };
    let down = CollectionFill {
        velocity: 1_600.0,
        limit: Some(2),
        ..Default::default()
    };
    let mut once = Harness::new(1_000, false, false);
    let mut split = Harness::new(1_000, false, false);
    for h in [&mut once, &mut split] {
        h.send(h.feedback(3200.0));
    }
    for top in [3840.0, 4400.0, 5000.0] {
        let before = rows(&split);
        once.send_filled(once.feedback(top), down);
        split.send_filled(
            split.feedback(top),
            CollectionFill {
                create_only: true,
                ..down
            },
        );
        let built = rows(&split);
        assert!(
            before.iter().all(|i| built.contains(i)),
            "a build-only report destroys nothing: {before:?} -> {built:?}"
        );
        assert!(split.snapshot().pending, "it leaves the retirement owed");
        let mut f = split.feedback(top);
        f.scroll_sequence -= 1;
        split.send_filled(
            f,
            CollectionFill {
                no_build: true,
                ..down
            },
        );
        assert_eq!(rows(&split), rows(&once), "at {top}");
    }
}

#[test]
fn a_build_only_report_that_changes_the_port_is_refused() {
    let mut h = Harness::new(1_000, false, false);
    h.send(h.feedback(3200.0));
    let mut f = h.feedback(3300.0);
    f.port_main += 10.0;
    let bytes = f
        .encode_with(CollectionFill {
            create_only: true,
            ..Default::default()
        })
        .unwrap();
    assert!(CollectionFeedback::decode_with_fill(&bytes).is_ok());
    assert!(h
        .try_send_filled(
            f,
            CollectionFill {
                create_only: true,
                ..Default::default()
            }
        )
        .is_err());
}

#[test]
fn noise_is_small_and_never_crosses_zero() {
    // Float32 ulps on a real height are noise; anything across zero is a
    // measurement (a zero-height row is skipped and revived as one).
    let mut index = SizeIndex::new(32.0).unwrap();
    index
        .replace_keys(["a", "b"].map(Into::into).to_vec())
        .unwrap();
    for (i, h) in [58.0, 0.0].into_iter().enumerate() {
        let token = index.measurement_token_at(i).unwrap();
        index.set_measured_height_at(i, token, h).unwrap();
    }
    assert!(index.noise_at(0, 57.999_97));
    assert_eq!(index.denoised(0, 57.999_97), 58.0);
    assert!(!index.noise_at(0, 58.02));
    assert!(index.noise_at(1, 0.0));
    assert!(!index.noise_at(1, 0.005));
    assert_eq!(index.denoised(1, 0.005), 0.005);
}

#[test]
fn a_window_that_leans_keeps_half_a_viewport_behind_its_travel() {
    let rows = |lean: bool, velocity: f64| {
        let mut h = Harness::new(1_000, false, false);
        h.send(h.feedback(3200.0));
        h.send_filled(
            h.feedback(3840.0),
            CollectionFill {
                velocity,
                lean,
                ..Default::default()
            },
        );
        let rows = h.snapshot().rows;
        (rows[0].index, rows[rows.len() - 1].index)
    };
    // 320 px of port, 32 px rows, the view at rows 120 to 129.
    let (first, last) = rows(false, 1_600.0);
    assert!((109..=110).contains(&first), "a viewport behind: {first}");
    let (lean_first, lean_last) = rows(true, 1_600.0);
    assert!((114..=115).contains(&lean_first), "half: {lean_first}");
    assert_eq!(lean_last, last, "the same ahead");
    // Toward the start it is the end side that is behind.
    let (up_first, up_last) = rows(true, -1_600.0);
    assert!((134..=135).contains(&up_last), "{up_last}");
    assert_eq!(up_first, rows(false, -1_600.0).0);
    // Without a velocity nothing leans.
    assert_eq!(rows(true, 0.0), rows(false, 0.0));
}

/// A report's padding takes effect with it, and only if it is accepted (@ref
/// LLP 1010 §6.9): a stale report's, or a refused build-only one's, leaves
/// the padding as the last accepted report left it, and a request after them
/// aligns within the scroll padding the list's style has.
#[test]
fn a_rejected_report_s_padding_changes_nothing() {
    let mut h = Harness::new(1_000, false, false);
    let padded = |f: CollectionFeedback, n| CollectionFeedback {
        padding: [n, n],
        ..f
    };
    let stale = padded(h.feedback(0.0), 0.0);
    let mut current = padded(h.feedback(0.0), 20.0);
    current.scroll_sequence += 1;
    assert!(h.send(current));
    assert_eq!(h.collection().padding(), [20.0, 20.0]);
    assert!(!h.try_send_filled(stale, CollectionFill::default()).unwrap());
    assert_eq!(h.collection().padding(), [20.0, 20.0], "a stale report");
    let mut resized = padded(h.feedback(0.0), 0.0);
    resized.port_main += 1.0;
    let build_only = CollectionFill {
        create_only: true,
        ..Default::default()
    };
    assert!(h.try_send_filled(resized, build_only).is_err());
    assert_eq!(h.collection().padding(), [20.0, 20.0], "a refused report");
    // A later accepted report takes only its own padding.
    h.send(padded(h.feedback(0.0), 20.0));
    assert_eq!(h.collection().padding(), [20.0, 20.0], "the next report");
    // A request after them aligns within the list's style's scroll padding,
    // which no report carries
    let sites = crate::instance::SiteIndex::new(&h.plan);
    let mut u = Update::new(env(&h.plan, &h.slots), &sites, &mut h.ids);
    let style = |_| [12.0, 0.0, 0.0, 0.0];
    u.scroll_padding = Some(&style);
    let Child::Node(root) = &mut h.tree.children[0] else {
        panic!()
    };
    let list = root.collection.as_mut().unwrap();
    list.begin_into_view(&mut u, &[], "n:50", Align::Start, false)
        .unwrap();
    let offset = list.correction.unwrap().offset;
    assert_eq!(offset, list.index.prefix(50).unwrap() - 12.0);
}

#[test]
fn padding_only_feedback_advances_revision_and_fences_same_token_reports() {
    for horizontal in [false, true] {
        let mut r = crate::Runner::boot(
            plan_opening(100, false, false, false, false, horizontal),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.collection_feedback(runner_feedback(&r)).unwrap();
        let before = r.collections()[0].clone();
        let mut old = runner_feedback(&r);
        old.scroll_sequence = before.scroll_sequence;
        let mut padded = old.clone();
        padded.padding = [0.0, 100.0];
        let accepted = r.collection_feedback(padded.clone()).unwrap();
        let after = r.collections()[0].clone();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(accepted.receipts.len(), 1, "publish the new revision");
        assert_eq!(after.rows, before.rows);
        assert_eq!(after.total_extent, before.total_extent);
        assert_eq!(after.correction, before.correction);
        assert_eq!(after.scroll_sequence, before.scroll_sequence);
        assert!(r.collection_feedback(old).unwrap().receipts.is_empty());
        assert_eq!(r.collections()[0], after);
        padded.revision = after.revision;
        assert!(r.collection_feedback(padded).unwrap().receipts.is_empty());
        r.scroll_into_view(IntoView {
            list: String::new(),
            key: Value::Number(99.0),
            block: Align::Start,
            inline: Align::Start,
            row: None,
            view: Some(after.view),
            smooth: false,
        })
        .unwrap();
        assert_eq!(r.collections()[0].correction.unwrap().offset, 2980.0);
    }
}

#[test]
fn padded_extent_overflow_is_rejected_before_any_geometry_mutation() {
    for horizontal in [false, true] {
        for variant in 0..6 {
            let mut h = Harness::from_parts(
                plan_opening(5, false, false, false, false, horizontal),
                vec![
                    values(5),
                    Value::Number(0.0),
                    Value::Number(0.0),
                    Value::Number(16.0),
                ],
            );
            h.send(h.feedback(0.0));
            let mut measured = h.feedback(0.0);
            measured.measurements = h
                .snapshot()
                .rows
                .iter()
                .map(|row| RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    size: if row.index == 0 { 1e38 } else { 0.0 },
                })
                .collect();
            h.send(measured);
            if variant == 5 {
                let Child::Node(root) = &mut h.tree.children[0] else {
                    panic!()
                };
                root.collection.as_mut().unwrap().estimated_height = 1e38;
            }
            let before = h.snapshot();
            let fingerprint = h.collection().index.fingerprint();
            let geometry = h.collection().geometry.clone();
            let ids = h.ids.next;
            let mut f = h.feedback(before.correction.map_or(0.0, |c| c.offset));
            f.padding = if variant == 0 {
                [f32::MAX as f64, 0.0]
            } else {
                [0.0, f32::MAX as f64]
            };
            if variant == 2 {
                f.cross += 1.0;
            }
            if variant == 3 {
                let row = before.rows.iter().find(|row| row.index == 0).unwrap();
                f.measurements.push(RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    size: 2e38,
                });
                f.padding = [0.0, 2e38];
            }
            if variant == 5 {
                f.cross += 1.0;
                f.padding = [0.0; 2];
            }
            let fill = CollectionFill {
                create_only: variant == 4,
                ..Default::default()
            };
            assert!(
                matches!(
                    h.try_send_filled(f, fill),
                    Err(InstanceError::InvalidCollectionFeedback)
                ),
                "variant {variant}"
            );
            assert_eq!(h.snapshot(), before);
            assert_eq!(h.collection().index.fingerprint(), fingerprint);
            assert_eq!(h.collection().geometry, geometry);
            assert_eq!(h.ids.next, ids);
            assert_eq!(h.collection().padding(), [0.0; 2]);
            if variant == 3 {
                // Validate the final extent: a smaller measurement can make
                // padding admissible even though the old extent plus it is not.
                let row = before.rows.iter().find(|row| row.index == 0).unwrap();
                let mut valid = h.feedback(0.0);
                valid.padding = [0.0, 3e38];
                valid.measurements.push(RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    size: 1e37,
                });
                assert!(h.send(valid));
                assert!(h.collection().index.scroll_extent() <= f32::MAX as f64);
                assert!(h
                    .snapshot()
                    .correction
                    .is_none_or(|c| c.offset.abs() <= f32::MAX as f64));
            }
        }
    }
}

#[test]
fn aggregate_preview_matches_binary_sums_and_both_padding_orders() {
    let initial = [
        3.855634828180998e37,
        4.110381270879275e37,
        1.2150761836965103e37,
        1.4554140070018447e37,
    ];
    let next = [
        7.162362434933901e36,
        1.743078113874182e37,
        1.85932241262857e37,
        3.2119624082087986e37,
    ];
    for horizontal in [false, true] {
        for variant in 0..3 {
            let mut r = crate::Runner::boot(
                plan_opening(
                    if variant == 0 { 4 } else { 1 },
                    false,
                    true,
                    false,
                    true,
                    horizontal,
                ),
                NoData,
                Kernel::with_monospace(),
                Default::default(),
                "/",
            )
            .unwrap();
            let report = |r: &crate::Runner<NoData>, sizes: &[f64]| {
                let mut f = runner_feedback(r);
                f.port_main = 1e38;
                f.offset = r.collections()[0].correction.map_or(0.0, |c| c.offset);
                f.measurements = r.collections()[0]
                    .rows
                    .iter()
                    .filter_map(|row| {
                        sizes.get(row.index).map(|&size| RowMeasurement {
                            view: row.view,
                            epoch: row.epoch,
                            size,
                        })
                    })
                    .collect();
                f
            };
            r.collection_feedback(report(&r, &[])).unwrap();
            r.collection_feedback(report(
                &r,
                if variant == 0 {
                    &initial
                } else {
                    &[1.3857361873091886e38]
                },
            ))
            .unwrap();
            let before = r.collections();
            let live = r.kernel().live_count();
            let mut f = report(&r, if variant == 0 { &next } else { &[] });
            f.port_main = 600.0;
            f.padding = match variant {
                0 => [0.0, 2.6497635485647948e38],
                1 => [1.383018962808564e37, 1.878785382795244e38],
                _ => [1.878785382795244e38, 1.383018962808564e37],
            };
            assert!(f.encode().is_ok(), "every individual number fits f32");
            assert!(
                matches!(
                    r.collection_feedback(f),
                    Err(crate::RunnerError::Instance(
                        InstanceError::InvalidCollectionFeedback
                    ))
                ),
                "variant {variant}/{horizontal}"
            );
            assert!(!r.is_poisoned());
            assert_eq!(r.collections(), before);
            assert_eq!(r.kernel().live_count(), live);
            r.collection_feedback(report(&r, &[])).unwrap();
            assert!(r.collections()[0]
                .correction
                .is_none_or(|c| c.offset.abs() <= f32::MAX as f64));
        }
    }
}
