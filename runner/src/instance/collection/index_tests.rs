use super::*;

#[test]
fn reorder_gap_requires_current_measured_neighbors_and_right_biases_zero_ties() {
    let mut i = index(&[20., 0., 0., 30., 10.]);
    assert_eq!(i.certified_gap(19.).unwrap(), Some(3));
    assert_eq!(i.certified_gap(20.).unwrap(), Some(3));
    assert_eq!(i.certified_gap(60.).unwrap(), Some(5));
    i.invalidate_row("k1").unwrap();
    assert_eq!(i.certified_gap(20.).unwrap(), None);
    i.invalidate_all().unwrap();
    assert_eq!(i.certified_gap(60.).unwrap(), None);
}

#[test]
fn reorder_twenty_five_thousand_zero_ties_do_not_scan_rows() {
    let mut heights = vec![0.; 25_000];
    heights[0] = 20.;
    heights[24_999] = 20.;
    let mut i = index(&heights);
    i.tree.visits.set(0);
    assert_eq!(i.certified_gap(20.).unwrap(), Some(24_999));
    assert!(i.tree.visits.get() < 200);
    i.tree.visits.set(0);
    assert_eq!(i.certified_gap_excluding(5., 0).unwrap(), Some(24_999));
    assert!(i.tree.visits.get() < 200);
    i.invalidate_row("k12000").unwrap();
    i.tree.visits.set(0);
    assert_eq!(i.certified_gap(20.).unwrap(), None);
    assert!(i.tree.visits.get() < 200);
}

fn keys(count: usize) -> Vec<Rc<str>> {
    (0..count).map(|i| Rc::from(format!("k{i}"))).collect()
}

fn index(heights: &[f64]) -> HeightIndex {
    let mut index = HeightIndex::new(10.0).unwrap();
    index.replace_keys(keys(heights.len())).unwrap();
    for (i, &height) in heights.iter().enumerate() {
        let key = format!("k{i}");
        let token = index.measurement_token(&key).unwrap();
        assert!(index.set_measured_height(&key, token, height).unwrap());
    }
    index
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-8, "{actual} != {expected}");
}

fn linear_row(heights: &[f64], offset: f64) -> Option<usize> {
    let mut sum = 0.0;
    heights.iter().position(|height| {
        sum += height;
        sum > offset
    })
}

fn linear_band(heights: &[f64], start: f64, end: f64) -> Range<usize> {
    let first = linear_row(heights, start).unwrap_or(heights.len());
    if start >= end {
        return first..first;
    }
    let mut sum = 0.0;
    let last = heights.iter().position(|height| {
        sum += height;
        sum >= end
    });
    first..last.map_or(heights.len(), |i| i + 1)
}

fn linear_segments(
    heights: &[f64],
    start: f64,
    end: f64,
    pins: [Option<usize>; 2],
) -> Vec<Range<usize>> {
    let mut segments: Vec<Range<usize>> = Vec::new();
    let mut top = 0.0;
    for (i, &height) in heights.iter().enumerate() {
        let bottom = top + height;
        let intersects = start < end && height > 0.0 && bottom > start && top < end;
        top = bottom;
        if !intersects && !pins.contains(&Some(i)) {
            continue;
        }
        if let Some(last) = segments.last_mut() {
            if last.end == i {
                last.end += 1;
                continue;
            }
        }
        segments.push(i..i + 1);
    }
    segments
}

struct Random(u64);

impl Random {
    fn next(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

#[test]
fn estimated_heights_and_key_rebuild_retain_measurements() {
    let mut index = HeightIndex::new(12.5).unwrap();
    index.replace_keys(keys(3)).unwrap();
    assert_eq!(index.len(), 3);
    assert_eq!(index.total_height(), 37.5);
    assert_eq!(index.key(2), Some("k2"));
    assert_eq!(index.key(3), None);
    assert_eq!(index.position("k1"), Some(1));
    assert_eq!(index.position("missing"), None);
    assert_eq!(index.prefix(4), None);
    assert!(!index.is_measured("k0"));
    let token = index.measurement_token("k0").unwrap();
    index.set_measured_height("k0", token, 21.0).unwrap();
    index
        .replace_keys(vec!["k2".into(), "new".into(), "k0".into()])
        .unwrap();
    assert_eq!(index.height(0), Some(12.5));
    assert_eq!(index.height(1), Some(12.5));
    assert_eq!(index.height(2), Some(21.0));
    assert_eq!(index.measurement_token("k0"), Some(token));
    assert!(index.is_measured("k0"));
    assert!(!index.is_measured("new"));
    assert_eq!(index.total_height(), 46.0);
}

#[test]
fn zero_heights_and_exact_boundaries_have_no_linear_search() {
    let index = index(&[0.0, 0.0, 10.0, 0.0, 20.0, 0.0, 0.0]);
    assert_eq!(index.row_at(0.0).unwrap(), Some(2));
    assert_eq!(index.row_at(9.0).unwrap(), Some(2));
    assert_eq!(index.row_at(10.0).unwrap(), Some(4));
    assert_eq!(index.row_at(30.0).unwrap(), None);
    assert_eq!(index.window(0.0, 10.0, [None; 2]).unwrap().visible, 2..3);
    assert_eq!(index.window(10.0, 10.0, [None; 2]).unwrap().visible, 4..5);
    assert_eq!(index.window(0.0, 30.0, [None; 2]).unwrap().visible, 2..5);
    let all_zero = index_with_zeros(100_000);
    assert_eq!(all_zero.row_at(0.0).unwrap(), None);
    let window = all_zero.window(0.0, 100.0, [None; 2]).unwrap();
    assert!(window.visible.is_empty());
    assert!(window.segments.is_empty());
    assert_eq!(all_zero.tree.visits.get(), 0);
}

fn index_with_zeros(count: usize) -> HeightIndex {
    let mut index = HeightIndex::new(0.0).unwrap();
    index.replace_keys(keys(count)).unwrap();
    index
}

#[test]
fn measured_interior_zero_run_keeps_25k_window_and_pins_bounded() {
    let mut heights = vec![0.0; 25_000];
    heights[0] = 10.0;
    heights[24_999] = 10.0;
    let mut index = index(&heights);
    assert!(index.is_measured("k12500"));
    let depth = index.tree.base.ilog2() as usize;
    index.tree.visits.set(0);
    let window = index.window(0.0, 10.0, [None; 2]).unwrap();
    assert_eq!(window.overscan, 0..25_000);
    assert_eq!(window.segments, vec![0..1, 24_999..25_000]);
    assert!(index.tree.visits.get() <= 12 * (depth + 1));
    let pinned = index
        .window(0.0, 10.0, [Some("k1000"), Some("k24000")])
        .unwrap();
    assert_eq!(
        pinned.segments,
        vec![0..1, 1000..1001, 24_000..24_001, 24_999..25_000]
    );
    // Retained zero estimates stay out until a fresh measurement gives them area.
    index.invalidate_all().unwrap();
    assert_eq!(index.window(0.0, 10.0, [None; 2]).unwrap(), window);
    let token = index.measurement_token("k12500").unwrap();
    index.set_measured_height("k12500", token, 10.0).unwrap();
    assert_eq!(
        index.window(0.0, 30.0, [None; 2]).unwrap().segments,
        vec![0..1, 12_500..12_501, 24_999..25_000]
    );
}

#[test]
fn randomized_updates_and_geometry_match_linear_oracle() {
    let mut rng = Random(0xdab5_21c4_a739_0351);
    for count in [0, 1, 2, 3, 31, 128, 257] {
        // Quarter-pixel units keep the independent oracle's sums exact.
        let mut heights: Vec<f64> = (0..count).map(|_| rng.next(100) as f64 / 4.0).collect();
        let mut index = index(&heights);
        for _ in 0..400 {
            if count > 0 {
                let row = rng.next(count);
                let key = format!("k{row}");
                let height = rng.next(200) as f64 / 4.0;
                let token = index.measurement_token(&key).unwrap();
                index.set_measured_height(&key, token, height).unwrap();
                heights[row] = height;
            }
            let total: f64 = heights.iter().sum();
            near(index.total_height(), total);
            for end in 0..=count {
                let prefix: f64 = heights[..end].iter().sum();
                near(index.prefix(end).unwrap(), prefix);
                assert_eq!(index.row_at(prefix).unwrap(), linear_row(&heights, prefix));
            }
            let offset = rng.next((total as usize + 1) * 4 + 100) as f64 / 4.0;
            let viewport = rng.next(401) as f64 / 4.0;
            assert_eq!(index.row_at(offset).unwrap(), linear_row(&heights, offset));
            let pins =
                std::array::from_fn(|_| (count > 0 && rng.next(3) == 0).then(|| rng.next(count)));
            let pin_keys = pins.map(|pin| pin.map(|i| format!("k{i}")));
            let window = index
                .window(
                    offset,
                    viewport,
                    [pin_keys[0].as_deref(), pin_keys[1].as_deref()],
                )
                .unwrap();
            let clamped = offset.min((total - viewport).max(0.0));
            assert_eq!(window.offset, clamped);
            let end = (clamped + viewport).min(total);
            assert_eq!(window.visible, linear_band(&heights, clamped, end));
            let band_start = (clamped - viewport).max(0.0);
            let band_end = (end + viewport).min(total);
            assert_eq!(window.overscan, linear_band(&heights, band_start, band_end));
            let expected = linear_segments(&heights, band_start, band_end, pins);
            assert_eq!(window.segments, expected);
        }
    }
}

#[test]
fn fractional_heights_do_not_accumulate_delta_drift() {
    let mut index = index(&[0.1, 0.2, 0.3, 0.4, 0.5]);
    let token = index.measurement_token("k1").unwrap();
    for _ in 0..10_000 {
        index.set_measured_height("k1", token, 12345.6789).unwrap();
        index.set_measured_height("k1", token, 0.2).unwrap();
    }
    near(index.total_height(), 1.5);
    near(index.prefix(3).unwrap(), 0.6);
    assert_eq!(index.row_at(0.31).unwrap(), Some(2));
}

#[test]
fn rounded_tree_boundaries_are_monotone_and_zero_rows_stay_empty() {
    let heights = [1e16, 0.0, 0.0, 0.0, 3.0, 0.0, 2.0, 0.0, 1e16];
    let index = index(&heights);
    let prefixes: Vec<_> = (0..=heights.len())
        .map(|i| index.prefix(i).unwrap())
        .collect();
    for i in 0..heights.len() {
        assert!(prefixes[i] <= prefixes[i + 1], "{prefixes:?}");
        if heights[i] == 0.0 {
            assert_eq!(prefixes[i], prefixes[i + 1]);
        }
    }
    for &offset in &prefixes {
        assert_eq!(
            index.row_at(offset).unwrap(),
            (0..heights.len()).find(|&i| prefixes[i + 1] > offset)
        );
    }
}

#[test]
fn pins_are_disjoint_deduplicated_and_bounded() {
    let index = index(&vec![10.0; 10_000]);
    let window = index
        .window(50_000.0, 100.0, [Some("k0"), Some("k9999")])
        .unwrap();
    assert_eq!(window.visible, 5000..5010);
    assert_eq!(window.overscan, 4990..5020);
    assert_eq!(window.segments, vec![0..1, 4990..5020, 9999..10000]);
    assert_eq!(
        window
            .segments
            .iter()
            .map(|range| range.len())
            .sum::<usize>(),
        32
    );
    assert_eq!(
        index
            .window(50_000.0, 100.0, [Some("k0"); 2])
            .unwrap()
            .segments,
        vec![0..1, 4990..5020]
    );
    assert_eq!(
        index
            .window(50_000.0, 100.0, [Some("k4989"), Some("k5020")])
            .unwrap()
            .segments,
        vec![4989..5021]
    );
    assert_eq!(
        index
            .window(50_000.0, 100.0, [Some("deleted"), Some("k5000")])
            .unwrap()
            .segments,
        vec![4990..5020]
    );
    let hidden = index
        .window(50_000.0, 0.0, [Some("k3"), Some("k9000")])
        .unwrap();
    assert!(hidden.visible.is_empty());
    assert_eq!(hidden.segments, vec![3..4, 9000..9001]);
}

#[test]
fn width_and_content_epochs_reject_stale_reports_without_losing_estimates() {
    let mut index = index(&[11.0, 22.0, 33.0]);
    let old = index.measurement_token("k1").unwrap();
    let rows = index.rows.clone();
    let tree = index.tree.sums.clone();
    let allocation = index.rows.as_ptr();
    index.tree.visits.set(0);
    index.invalidate_all().unwrap();
    assert_eq!(index.rows, rows);
    assert_eq!(index.rows.as_ptr(), allocation);
    assert_eq!(index.tree.sums, tree);
    assert_eq!(index.tree.visits.get(), 0);
    assert!(!index.is_measured("k1"));
    assert!(!index.set_measured_height("k1", old, 222.0).unwrap());
    let fresh = index.measurement_token("k1").unwrap();
    assert_ne!(fresh, old);
    assert!(index.set_measured_height("k1", fresh, 24.0).unwrap());
    assert!(index.is_measured("k1"));
    let next = index.invalidate_row("k1").unwrap();
    assert_ne!(next, fresh);
    assert_eq!(index.height(1), Some(24.0));
    assert!(!index.is_measured("k1"));
    assert!(!index.set_measured_height("k1", fresh, 300.0).unwrap());
    assert!(index.set_measured_height("k1", next, 27.0).unwrap());
    assert_eq!(index.height(0), Some(11.0));
    assert!(index.invalidate_row("missing").is_err());
}

#[test]
fn measurement_identity_survives_reorder_but_not_delete_and_reinsert() {
    let mut index = index(&[10.0, 20.0, 30.0]);
    let token = index.measurement_token("k1").unwrap();
    assert!(!index.set_measured_height("k0", token, 99.0).unwrap());
    index.replace_keys(vec!["k2".into(), "k1".into()]).unwrap();
    assert!(index.set_measured_height("k1", token, 21.0).unwrap());
    index.replace_keys(vec!["k2".into()]).unwrap();
    assert!(!index.set_measured_height("k1", token, 22.0).unwrap());
    index.replace_keys(vec!["k1".into(), "k2".into()]).unwrap();
    assert_ne!(index.measurement_token("k1"), Some(token));
    assert!(!index.set_measured_height("k1", token, 23.0).unwrap());
    assert_eq!(index.height(0), Some(10.0));
}

#[test]
fn anchors_preserve_key_and_offset_through_prepend_reorder_and_height_changes() {
    let mut index = index(&[10.0, 20.0, 30.0, 40.0, 50.0]);
    let anchor = index.capture_anchor(35.0, 20.0, false).unwrap();
    assert!(Rc::ptr_eq(&anchor.order, &index.order));
    index
        .replace_keys(vec![
            "new".into(),
            "k0".into(),
            "k1".into(),
            "k2".into(),
            "k3".into(),
            "k4".into(),
        ])
        .unwrap();
    assert_eq!(index.restore_anchor(&anchor, 20.0).unwrap(), 45.0);
    let token = index.measurement_token("k0").unwrap();
    index.set_measured_height("k0", token, 25.0).unwrap();
    assert_eq!(index.restore_anchor(&anchor, 20.0).unwrap(), 60.0);
    index.invalidate_all().unwrap();
    assert_eq!(index.restore_anchor(&anchor, 20.0).unwrap(), 60.0);
    index
        .replace_keys(vec![
            "k4".into(),
            "k3".into(),
            "k2".into(),
            "k1".into(),
            "k0".into(),
        ])
        .unwrap();
    assert_eq!(index.restore_anchor(&anchor, 20.0).unwrap(), 95.0);
}

#[test]
fn deleted_anchor_chooses_next_survivor_in_old_order_then_previous() {
    let mut index = index(&[10.0; 10]);
    let anchor = index.capture_anchor(32.0, 10.0, false).unwrap();
    index
        .replace_keys(vec!["k9".into(), "k7".into(), "k1".into(), "k0".into()])
        .unwrap();
    // k7 is the first old successor still present, even though k9 comes first now.
    assert_eq!(index.restore_anchor(&anchor, 10.0).unwrap(), 12.0);
    index
        .replace_keys(vec!["k0".into(), "k1".into(), "other".into()])
        .unwrap();
    assert_eq!(index.restore_anchor(&anchor, 10.0).unwrap(), 12.0);
    index
        .replace_keys(vec!["replacement".into(), "other".into()])
        .unwrap();
    assert_eq!(index.restore_anchor(&anchor, 10.0).unwrap(), 0.0);
    index.replace_keys(Vec::new()).unwrap();
    assert_eq!(index.restore_anchor(&anchor, 10.0).unwrap(), 0.0);
}

#[test]
fn end_follow_only_when_requested_and_already_at_end() {
    let mut index = index(&[10.0; 10]);
    let follow = index.capture_anchor(80.0, 20.0, true).unwrap();
    let preserve = index.capture_anchor(80.0, 20.0, false).unwrap();
    let reading = index.capture_anchor(20.0, 20.0, true).unwrap();
    index.replace_keys(keys(20)).unwrap();
    assert_eq!(index.restore_anchor(&follow, 30.0).unwrap(), 170.0);
    assert_eq!(index.restore_anchor(&preserve, 20.0).unwrap(), 80.0);
    assert_eq!(index.restore_anchor(&reading, 20.0).unwrap(), 20.0);
    assert_eq!(index.restore_anchor(&follow, 1000.0).unwrap(), 0.0);
    index.replace_keys(keys(3)).unwrap();
    assert_eq!(index.restore_anchor(&preserve, 20.0).unwrap(), 10.0);
    let mut empty = index_with_zeros(0);
    let anchor = empty.capture_anchor(0.0, 20.0, true).unwrap();
    empty.replace_keys(keys(3)).unwrap();
    assert_eq!(empty.restore_anchor(&anchor, 20.0).unwrap(), 0.0);
}

#[test]
fn end_follow_accepts_native_document_rounding() {
    let mut index = index(&[324_242.100_130_000_04]);
    let viewport = 502.0;
    let native_bottom = 323_740.1;
    assert!(native_bottom < index.max_offset(viewport));
    let following = index.capture_anchor(native_bottom, viewport, true).unwrap();
    let reading = index
        .capture_anchor(native_bottom, viewport, false)
        .unwrap();
    assert!(following.follows_end);
    assert!(!reading.follows_end);
    index.replace_keys(keys(2)).unwrap();
    assert_eq!(
        index.restore_anchor(&following, viewport).unwrap(),
        index.max_offset(viewport)
    );
    assert_eq!(
        index.restore_anchor(&reading, viewport).unwrap(),
        native_bottom
    );
}

#[test]
fn end_follow_accepts_integer_dom_scroll_height_rounding() {
    // The real DOM controller fixture supplies these facts: measured wrappers
    // total 335377.078125, scrollHeight 335377, clientHeight 519, scrollTop
    // 334858. The user has reached the actual DOM end, not an estimated tail.
    let mut index = index(&[335_080.0, 297.078_125]);
    let viewport = 519.0;
    let dom_bottom = 334_858.0;
    assert_eq!(index.max_offset(viewport) - dom_bottom, 0.078_125);
    let anchor = index.capture_anchor(dom_bottom, viewport, true).unwrap();
    let reader = index
        .capture_anchor(index.max_offset(viewport) - 0.500_001, viewport, true)
        .unwrap();
    let disabled = index.capture_anchor(dom_bottom, viewport, false).unwrap();
    assert!(
        anchor.follows_end,
        "integer DOM end must follow appended content"
    );
    assert!(
        !reader.follows_end,
        "reader farther than half a pixel stays anchored"
    );
    assert!(!disabled.follows_end);
    index.replace_keys(keys(3)).unwrap();
    assert_eq!(
        index.restore_anchor(&anchor, viewport).unwrap(),
        index.max_offset(viewport)
    );
    assert_eq!(
        index.restore_anchor(&disabled, viewport).unwrap(),
        dom_bottom
    );
}

#[test]
fn end_follow_accepts_browser_integer_scroll_range_rounding() {
    // Observed in Messages stress: CSS rows sum to a fractional extent, while
    // Chrome clamps an authored scrollTop to its integer scroll range.
    let mut index = index(&[10_497.109_375]);
    let anchor = index.capture_anchor(10_023.0, 474.0, true).unwrap();
    assert!(anchor.follows_end);
    index.replace_keys(keys(2)).unwrap();
    assert_eq!(
        index.restore_anchor(&anchor, 474.0).unwrap(),
        index.max_offset(474.0)
    );
}

#[test]
fn end_follow_rounding_tolerance_is_half_a_logical_pixel_on_every_host() {
    // The .5 boundary is inclusive at small, medium and large extents alike.
    for (height, viewport) in [(100.0, 20.0), (1_048_576.0, 512.0), (268_435_456.0, 512.0)] {
        let mut index = index(&[height]);
        let maximum = index.max_offset(viewport);
        let boundary = maximum - 0.5;
        let outside = boundary - 0.000_001;
        let following = index.capture_anchor(boundary, viewport, true).unwrap();
        let reading = index.capture_anchor(outside, viewport, true).unwrap();
        assert!(following.follows_end, "height={height}");
        assert!(!reading.follows_end, "height={height}");
        assert!(
            !index
                .capture_anchor(boundary, viewport, false)
                .unwrap()
                .follows_end
        );
        assert!(
            !index
                .capture_anchor(maximum, 0.0, true)
                .unwrap()
                .follows_end
        );
        index.replace_keys(keys(2)).unwrap();
        assert_eq!(
            index.restore_anchor(&following, viewport).unwrap(),
            index.max_offset(viewport)
        );
        assert_eq!(index.restore_anchor(&reading, viewport).unwrap(), outside);
    }
}

#[test]
fn invalid_numbers_and_duplicates_leave_index_unchanged() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        assert!(HeightIndex::new(bad).is_err());
        let mut index = index(&[10.0, 20.0]);
        let token = index.measurement_token("k0").unwrap();
        assert!(index.set_measured_height("k0", token, bad).is_err());
        assert_eq!(index.total_height(), 30.0);
        assert_eq!(index.measurement_token("k0"), Some(token));
        assert!(index.row_at(bad).is_err());
        assert!(index.window(bad, 10.0, [None; 2]).is_err());
        assert!(index.window(0.0, bad, [None; 2]).is_err());
        assert!(index.capture_anchor(bad, 10.0, false).is_err());
        assert!(index.capture_anchor(0.0, bad, false).is_err());
        let anchor = index.capture_anchor(0.0, 10.0, false).unwrap();
        assert!(index.restore_anchor(&anchor, bad).is_err());
    }
    let mut index = index(&[10.0, 20.0]);
    let order = Rc::clone(&index.order);
    let generation = index.next_generation;
    assert_eq!(
        index.replace_keys(vec!["new".into(), "new".into()]),
        Err(IndexError::DuplicateKey("new".into()))
    );
    assert!(Rc::ptr_eq(&index.order, &order));
    assert_eq!(index.next_generation, generation);
    assert_eq!(index.total_height(), 30.0);
    let token = index.measurement_token("k0").unwrap();
    index.set_measured_height("k0", token, -0.0).unwrap();
    assert_eq!(index.height(0).unwrap().to_bits(), 0.0_f64.to_bits());
}

#[test]
fn extent_overflow_is_transactional_and_large_geometry_saturates() {
    let mut index = index(&[f64::MAX / 2.0, f64::MAX / 4.0]);
    let token = index.measurement_token("k1").unwrap();
    let total = index.total_height();
    assert_eq!(
        index.set_measured_height("k1", token, f64::MAX),
        Err(IndexError::ExtentOverflow)
    );
    assert_eq!(index.total_height(), total);
    assert_eq!(index.height(1), Some(f64::MAX / 4.0));
    let window = index.window(f64::MAX, f64::MAX, [None; 2]).unwrap();
    assert_eq!(window.offset, 0.0);
    assert_eq!(window.segments, vec![0..2]);
    let mut huge = HeightIndex::new(f64::MAX).unwrap();
    huge.replace_keys(keys(1)).unwrap();
    let generation = huge.next_generation;
    assert_eq!(huge.replace_keys(keys(2)), Err(IndexError::ExtentOverflow));
    assert_eq!(huge.len(), 1);
    assert_eq!(huge.next_generation, generation);
}

#[test]
fn generation_overflow_does_not_revalidate_ancient_tokens() {
    let mut index = index(&[10.0, 20.0]);
    index.epoch = u64::MAX;
    assert_eq!(index.invalidate_all(), Err(IndexError::GenerationExhausted));
    assert_eq!(index.epoch, u64::MAX);
    index.next_generation = u64::MAX;
    let token = index.measurement_token("k0").unwrap();
    assert_eq!(
        index.invalidate_row("k0"),
        Err(IndexError::GenerationExhausted)
    );
    assert_eq!(index.measurement_token("k0"), Some(token));
    assert_eq!(
        index.replace_keys(keys(3)),
        Err(IndexError::GenerationExhausted)
    );
    assert_eq!(index.len(), 2);
}

#[test]
fn twenty_traversals_do_not_rebuild_or_accumulate_scroll_metadata() {
    let mut index = HeightIndex::new(10.0).unwrap();
    index.replace_keys(keys(25_000)).unwrap();
    let order = Rc::clone(&index.order);
    let rows = index.rows.as_ptr();
    let sums = index.tree.sums.as_ptr();
    let rebuilds = index.rebuilds;
    let generations = index.next_generation;
    let depth = index.tree.base.ilog2() as usize;
    for traversal in 0..20 {
        for step in 0..2500 {
            let offset = if traversal % 2 == 0 {
                step * 100
            } else {
                (2499 - step) * 100
            } as f64;
            index.tree.visits.set(0);
            let window = index
                .window(offset, 100.0, [Some("k0"), Some("k24999")])
                .unwrap();
            let anchor = index.capture_anchor(offset, 100.0, false).unwrap();
            assert_eq!(index.restore_anchor(&anchor, 100.0).unwrap(), window.offset);
            assert!(window.segments.len() <= 3);
            assert!(window.segments.iter().map(|r| r.len()).sum::<usize>() <= 32);
            assert!(index.tree.visits.get() <= 12 * (depth + 1));
            assert!(Rc::ptr_eq(&anchor.order, &order));
        }
    }
    assert!(Rc::ptr_eq(&index.order, &order));
    assert_eq!(Rc::strong_count(&order), 2);
    assert_eq!(index.rows.as_ptr(), rows);
    assert_eq!(index.tree.sums.as_ptr(), sums);
    assert_eq!(index.rebuilds, rebuilds);
    assert_eq!(index.next_generation, generations);
    assert_eq!(index.rows.len(), 25_000);
    assert_eq!(index.positions.len(), 25_000);
}

#[test]
fn twenty_25k_traversals_with_zero_samples_keep_selection_and_metadata_bounded() {
    let heights: Vec<_> = (0..25_000)
        .map(|i| {
            if i % 5 == 0 && !(2000..22_000).contains(&i) {
                10.0
            } else {
                0.0
            }
        })
        .collect();
    let index = index(&heights);
    let order = Rc::clone(&index.order);
    let rows = index.rows.as_ptr();
    let sums = index.tree.sums.as_ptr();
    let rebuilds = index.rebuilds;
    let generations = index.next_generation;
    let depth = index.tree.base.ilog2() as usize;
    for traversal in 0..20 {
        for step in 0..2500 {
            let step = if traversal % 2 == 0 {
                step
            } else {
                2499 - step
            };
            let offset = step as f64 * index.total_height() / 2500.0;
            index.tree.visits.set(0);
            let window = index
                .window(offset, 100.0, [Some("k10000"), Some("k12000")])
                .unwrap();
            let selected: Vec<_> = window.segments.iter().cloned().flatten().collect();
            assert!(selected.len() <= 33);
            assert!(selected.contains(&10_000));
            assert!(selected.contains(&12_000));
            assert!(selected.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(selected
                .iter()
                .all(|&i| heights[i] > 0.0 || i == 10_000 || i == 12_000));
            // Work scales with selected rows and tree depth, never the zero run.
            assert!(index.tree.visits.get() <= (12 + 4 * selected.len()) * (depth + 1));
            let anchor = index.capture_anchor(offset, 100.0, false).unwrap();
            assert_eq!(index.restore_anchor(&anchor, 100.0).unwrap(), window.offset);
            assert!(Rc::ptr_eq(&anchor.order, &order));
        }
    }
    assert!(Rc::ptr_eq(&index.order, &order));
    assert_eq!(Rc::strong_count(&order), 2);
    assert_eq!(index.rows.as_ptr(), rows);
    assert_eq!(index.tree.sums.as_ptr(), sums);
    assert_eq!(index.rebuilds, rebuilds);
    assert_eq!(index.next_generation, generations);
    assert_eq!(index.rows.len(), 25_000);
    assert_eq!(index.positions.len(), 25_000);
}

#[test]
fn source_excluded_gap_certifies_zero_run_even_for_upper_half_of_source() {
    let mut i = index(&[20., 20., 0., 20.]);
    assert_eq!(i.certified_gap_excluding(25., 1).unwrap(), Some(3));
    assert_eq!(i.certified_gap_excluding(35., 1).unwrap(), Some(3));
    i.invalidate_row("k2").unwrap();
    assert_eq!(i.certified_gap_excluding(25., 1).unwrap(), None);
    assert_eq!(i.certified_gap_excluding(35., 1).unwrap(), None);
}

#[test]
fn identical_order_preserves_index_allocations_generations_and_measurements() {
    for n in [0, 3, 25_000] {
        let heights: Vec<_> = (0..n).map(|i| if i % 3 == 0 { 0. } else { 12.5 }).collect();
        let mut index = index(&heights);
        if n > 0 {
            index.invalidate_row("k1").unwrap();
        }
        let order = Rc::clone(&index.order);
        let rows = index.rows.clone();
        let positions = index.positions.clone();
        let row_allocation = index.rows.as_ptr();
        let tree_allocation = index.tree.sums.as_ptr();
        let measured_allocation = index.tree.measured.as_ptr();
        let sums = index.tree.sums.clone();
        let measured = index.tree.measured.clone();
        let generation = index.next_generation;
        let epoch = index.epoch;
        let rebuilds = index.rebuilds;
        index.tree.visits.set(17);
        for _ in 0..3 {
            index.replace_keys(keys(n)).unwrap();
            assert_eq!(
                index.rebuilds, rebuilds,
                "identical order rebuilt the index"
            );
            assert!(Rc::ptr_eq(&index.order, &order));
            assert_eq!(index.rows.as_ptr(), row_allocation);
            assert_eq!(index.tree.sums.as_ptr(), tree_allocation);
            assert_eq!(index.tree.measured.as_ptr(), measured_allocation);
            assert_eq!(index.rows, rows);
            assert_eq!(index.positions, positions);
            assert_eq!(index.tree.sums, sums);
            assert_eq!(index.tree.measured, measured);
            assert_eq!(index.next_generation, generation);
            assert_eq!(index.epoch, epoch);
            assert_eq!(index.tree.visits.get(), 17);
        }
        if n > 0 {
            assert!(index.is_measured("k0"));
            assert!(!index.is_measured("k1"));
            assert!(index.is_measured("k2"));
        }
    }
}

#[test]
fn identical_order_fastpath_does_not_mask_reorder_insert_delete_or_duplicate() {
    let mut index = index(&[10., 20., 30.]);
    let old = index.measurement_token("k1").unwrap();
    let rebuilds = index.rebuilds;
    index
        .replace_keys(vec!["k2".into(), "k0".into(), "k1".into()])
        .unwrap();
    assert_eq!(index.rebuilds, rebuilds + 1);
    assert_eq!(index.measurement_token("k1"), Some(old));
    assert_eq!(index.height(2), Some(20.));
    let order = Rc::clone(&index.order);
    let generation = index.next_generation;
    let rows = index.rows.clone();
    let sums = index.tree.sums.clone();
    assert_eq!(
        index.replace_keys(vec!["k2".into(), "k0".into(), "k2".into()]),
        Err(IndexError::DuplicateKey("k2".into()))
    );
    assert_eq!(index.rebuilds, rebuilds + 1);
    assert!(Rc::ptr_eq(&index.order, &order));
    assert_eq!(index.next_generation, generation);
    assert_eq!(index.rows, rows);
    assert_eq!(index.tree.sums, sums);
    index.replace_keys(vec!["k2".into(), "k0".into()]).unwrap();
    assert_eq!(index.rebuilds, rebuilds + 2);
    index
        .replace_keys(vec!["k2".into(), "k0".into(), "k1".into()])
        .unwrap();
    assert_eq!(index.rebuilds, rebuilds + 3);
    assert_ne!(index.measurement_token("k1"), Some(old));
    assert!(!index.set_measured_height("k1", old, 90.).unwrap());
    assert_eq!(index.height(2), Some(10.));
}

#[test]
fn measured_bands_require_current_rows_without_scanning_the_band() {
    let mut i = index(&vec![1.; 25_000]);
    i.tree.visits.set(0);
    assert!(i.range_measured(0..25_000));
    assert!(i.tree.visits.get() < 100);
    i.invalidate_row("k12000").unwrap();
    assert!(!i.range_measured(0..25_000));
    assert!(i.range_measured(0..12_000));
    assert!(i.range_measured(12_001..25_000));
    assert!(!i.range_measured(0..0));
    i.invalidate_all().unwrap();
    assert!(!i.range_measured(0..12_000));
    let token = i.measurement_token("k0").unwrap();
    i.set_measured_height("k0", token, 0.).unwrap();
    assert!(i.range_measured(0..1));
    assert!(!i.range_measured(0..2));
}
