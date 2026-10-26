use super::*;
use serde_json::json;

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

#[test]
fn each_orientation_maps_the_stored_grid_upright() {
    // A 4 × 2 stored grid: where its stored top-left and top-right corners land.
    let corners = |o| {
        let m = orient(o, 4.0, 2.0);
        (m.apply(0.0, 0.0), m.apply(4.0, 0.0))
    };
    assert_eq!(corners(1), ((0.0, 0.0), (4.0, 0.0)));
    assert_eq!(corners(2), ((4.0, 0.0), (0.0, 0.0)));
    assert_eq!(corners(3), ((4.0, 2.0), (0.0, 2.0)));
    assert_eq!(corners(4), ((0.0, 2.0), (4.0, 2.0)));
    // 5–8 transpose: the upright picture is 2 × 4. Orientation 6's stored top
    // row is its right side, read downwards.
    assert_eq!(corners(5), ((0.0, 0.0), (0.0, 4.0)));
    assert_eq!(corners(6), ((2.0, 0.0), (2.0, 4.0)));
    assert_eq!(corners(7), ((2.0, 4.0), (2.0, 0.0)));
    assert_eq!(corners(8), ((0.0, 4.0), (0.0, 0.0)));
}

#[test]
fn a_quarter_turn_swaps_the_sides_and_resamples_nothing() {
    let edit = Edit {
        rotate: 90.0,
        ..Edit::default()
    };
    let g = Geometry::new("transformImage", 48, 64, &edit).unwrap();
    assert_eq!(g.turned, (64, 48));
    assert_eq!(g.crop, (0, 0, 64, 48));
    // The upright top-left corner turns to the top-right; the left edge to the top.
    assert!(close(g.map.apply(0.0, 0.0), (64.0, 0.0)));
    assert!(close(g.map.apply(0.0, 64.0), (0.0, 0.0)));
    assert_eq!(
        (g.map.a, g.map.b, g.map.c, g.map.d),
        (0.0, 1.0, -1.0, 0.0),
        "exact, so no resampling"
    );
    let half = Geometry::new(
        "transformImage",
        48,
        64,
        &Edit {
            rotate: 180.0,
            ..Edit::default()
        },
    )
    .unwrap();
    assert_eq!(half.turned, (48, 64));
    assert!(close(half.map.apply(0.0, 0.0), (48.0, 64.0)));
}

#[test]
fn a_free_turn_takes_its_bounding_box() {
    let g = Geometry::new(
        "transformImage",
        100,
        50,
        &Edit {
            rotate: 30.0,
            ..Edit::default()
        },
    )
    .unwrap();
    // 100 cos 30 + 50 sin 30 = 111.6; 100 sin 30 + 50 cos 30 = 93.3.
    assert_eq!(g.turned, (112, 94));
    // The centre stays the centre.
    assert!(close(g.map.apply(50.0, 25.0), (56.0, 47.0)));
}

#[test]
fn a_flip_mirrors_the_turned_frame() {
    let g = Geometry::new(
        "transformImage",
        48,
        64,
        &Edit {
            rotate: 90.0,
            flip: Some(Flip::Horizontal),
            crop: None,
        },
    )
    .unwrap();
    // Turned, the left edge is on top; mirrored, the top-left corner is where it was.
    assert!(close(g.map.apply(0.0, 0.0), (0.0, 0.0)));
    let v = Geometry::new(
        "transformImage",
        48,
        64,
        &Edit {
            flip: Some(Flip::Vertical),
            ..Edit::default()
        },
    )
    .unwrap();
    assert!(close(v.map.apply(0.0, 0.0), (0.0, 64.0)));
}

#[test]
fn a_crop_is_clipped_rounded_and_refused_when_it_misses() {
    let crop = |x, y, width, height| {
        Geometry::new(
            "compressImage",
            48,
            64,
            &Edit {
                crop: Some(Rect {
                    x,
                    y,
                    width,
                    height,
                }),
                ..Edit::default()
            },
        )
        .map(|g| g.crop)
    };
    assert_eq!(crop(0.0, 0.0, 24.0, 64.0), Ok((0, 0, 24, 64)));
    assert_eq!(crop(-10.0, 60.0, 30.0, 30.0), Ok((0, 60, 20, 4)));
    // Edges round, not sizes: a square frame stays within a pixel of square.
    assert_eq!(crop(0.4, 0.6, 10.2, 10.2), Ok((0, 1, 11, 10)));
    assert_eq!(crop(47.9, 0.0, 0.05, 1.0), Ok((47, 0, 1, 1)));
    assert_eq!(
        crop(48.0, 0.0, 10.0, 10.0),
        Err("compressImage: outside: the crop misses the 48×64 image".into())
    );
    assert!(crop(0.0, -5.0, 1.0, 5.0).is_err());
    // The crop is in the turned frame, which the map moves to its origin.
    let g = Geometry::new(
        "transformImage",
        48,
        64,
        &Edit {
            rotate: 90.0,
            crop: Some(Rect {
                x: 10.0,
                y: 0.0,
                width: 20.0,
                height: 48.0,
            }),
            flip: None,
        },
    )
    .unwrap();
    assert_eq!(g.size(), (20, 48));
    assert!(close(g.map.apply(0.0, 64.0), (-10.0, 0.0)));
    assert!(close(g.to_output((10, 24)).apply(0.0, 64.0), (-5.0, 0.0)));
}

#[test]
fn resizing_follows_create_image_bitmap() {
    assert_eq!(resized(64, 48, None, None), (64, 48));
    assert_eq!(resized(64, 48, Some(32), None), (32, 24));
    assert_eq!(resized(64, 48, None, Some(10)), (14, 10)); // 13.33 rounds up
    assert_eq!(resized(64, 48, Some(5), Some(500)), (5, 500));
}

#[test]
fn options_are_read_and_bad_ones_refused() {
    let t = parse_transform(&json!({"rotate": -90, "flip": "vertical", "crop": {"x": 1, "y": 2, "width": 3, "height": 4},
        "resizeWidth": 8, "resizeQuality": "pixelated", "type": "image/png", "quality": 0.5, "metadata": "no-location"}))
    .unwrap();
    assert_eq!(t.edit.rotate, 270.0);
    assert_eq!(t.edit.flip, Some(Flip::Vertical));
    assert_eq!(t.resize_width, Some(8));
    assert_eq!(t.resize_quality, ResizeQuality::Pixelated);
    assert_eq!(t.media_type.as_deref(), Some("image/png"));
    assert_eq!((t.quality, t.metadata), (0.5, Metadata::NoLocation));
    let unsaid = parse_transform(&json!({})).unwrap();
    assert_eq!(
        (unsaid.quality, unsaid.metadata, unsaid.resize_quality),
        (0.9, Metadata::Keep, ResizeQuality::High)
    );
    for bad in [
        json!({"rotate": "90"}),
        json!({"flip": "both"}),
        json!({"crop": {"x": 0, "y": 0, "width": 0, "height": 1}}),
        json!({"crop": [0, 0, 1, 1]}),
        json!({"resizeWidth": 0}),
        json!({"resizeHeight": 1.5}),
        json!({"quality": 2}),
        json!({"metadata": "some"}),
        json!({"resizeQuality": "best"}),
        json!(7),
    ] {
        let e = parse_transform(&bad).unwrap_err();
        assert!(e.starts_with("transformImage: invalid: "), "{bad}: {e}");
    }
    assert!(parse_edit("compressImage", &json!({"rotate": [90]}))
        .unwrap_err()
        .starts_with("compressImage: invalid: "));
}
