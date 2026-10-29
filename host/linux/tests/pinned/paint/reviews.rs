//! Regression crops for LLP 1149 S0's code reviews.
use super::*;
use exact_linux::paint::{Backend, Ink as Paint, Shape, SvgPaint};
use tiny_skia::Transform;

fn pair(size: (f32, f32), draw: impl Fn(&mut dyn Backend)) -> (Pixmap, Pixmap) {
    let mut cpu = exact_linux::raster::Raster::new();
    let mut ink = exact_linux::ink::Ink::new().expect("native Ink adapter");
    for painter in [&mut cpu as &mut dyn Backend, &mut ink] {
        painter.begin(size.0, size.1, 1.);
        draw(painter);
    }
    (cpu.finish().unwrap(), ink.finish().unwrap())
}

fn pixel_near(a: &Pixmap, b: &Pixmap, x: u32, y: u32, tolerance: u8) {
    let a = a.pixel(x, y).unwrap();
    let b = b.pixel(x, y).unwrap();
    assert!(
        a.red().abs_diff(b.red()) <= tolerance
            && a.green().abs_diff(b.green()) <= tolerance
            && a.blue().abs_diff(b.blue()) <= tolerance,
        "({x}, {y}): {a:?} != {b:?}"
    );
}

#[test]
fn inherited_fractional_clips_cover_each_group_child_once() {
    if !ink_available() {
        return;
    }
    for overflow in [false, true] {
        let (cpu, ink) = pair((12., 12.), |b| {
            let t = Transform::identity();
            b.push_clip(&Shape::rect((4.5, 0., 7.5, 12.)), t);
            if overflow {
                b.push_clip(&Shape::rect((0., 0.25, 12., 11.75)), t);
                b.push_clip(&Shape::rect((0., 0.25, 12., 11.75)), t);
            }
            b.push_opacity(0.75);
            for _ in 0..2 {
                b.fill(&Shape::rect((0., 0., 12., 12.)), [0, 0, 0, 255], t);
            }
            b.pop_opacity();
            if overflow {
                b.pop_clip();
                b.pop_clip();
            }
            b.pop_clip();
        });
        pixel_near(&cpu, &ink, 4, 6, 2);
    }
}

#[test]
fn inherited_clips_keep_hidden_children_out_of_backdrop_blur() {
    if !ink_available() {
        return;
    }
    use exact_kernel::style::{BackdropFilter, BackdropOp};
    let (cpu, ink) = pair((32., 16.), |b| {
        let t = Transform::identity();
        b.push_clip(&Shape::rect((16., 0., 16., 16.)), t);
        b.push_opacity(0.8);
        b.fill(&Shape::rect((0., 0., 16., 16.)), [255, 0, 0, 255], t);
        b.backdrop_filter(
            &Shape::rect((0., 0., 32., 16.)),
            &BackdropFilter(vec![BackdropOp::Blur(4.)]),
            t,
        );
        b.pop_opacity();
        b.pop_clip();
    });
    pixel_near(&cpu, &ink, 16, 8, 1);
}

#[test]
fn svg_dashes_and_phase_match_cpu() {
    if !ink_available() {
        return;
    }
    for phase in [0., 5.] {
        let (cpu, ink) = pair((80., 16.), |b| {
            let path = exact_kernel::svg::parse_d("M 0 8 H 80");
            b.svg_path(
                &SvgPaint {
                    path: &path,
                    fill: None,
                    stroke: Some(Paint::Solid([0, 0, 0, 255])),
                    even_odd: false,
                    order: [0, 1, 2],
                    width: 4.,
                    cap: 0,
                    join: 0,
                    miter: 4.,
                    dash: vec![8., 8.],
                    phase,
                },
                Transform::identity(),
            );
        });
        for x in 0..80 {
            pixel_near(&cpu, &ink, x, 8, 1);
        }
    }
}

#[test]
fn shrunken_shadow_reduces_radii_to_its_rectangle() {
    if !ink_available() {
        return;
    }
    let (cpu, ink) = pair((180., 100.), |b| {
        let shape = Shape {
            rect: (140., 40., 20., 20.),
            radii: [(40., 40.), (0., 0.), (0., 0.), (0., 0.)],
            corners: None,
        };
        if !b.blurred_shadow(
            &shape,
            [0, 0, 0, 255],
            0.,
            &Shape::rect((0., 0., 100., 100.)),
            Transform::identity(),
        ) {
            b.fill(
                &Shape::elliptical(shape.rect, shape.radii),
                [0, 0, 0, 255],
                Transform::identity(),
            );
        }
    });
    pixel_near(&cpu, &ink, 155, 45, 2);
    let crop = tiny_skia::IntRect::from_xywh(140, 40, 20, 20).unwrap();
    let (mean, over) = band(
        &cpu.clone_rect(crop).unwrap(),
        &ink.clone_rect(crop).unwrap(),
    );
    assert!(mean < 5. && over < 6., "shadow crop: {mean}, {over}%");
}

#[test]
fn gradient_preserves_narrow_intervals_and_hard_stops() {
    if !ink_available() {
        return;
    }
    use exact_kernel::{gradient::Geometry, Color};
    use exact_linux::paint::GradientPaint;
    for narrow in [true, false] {
        let (cpu, ink) = pair((2000., 1.), |b| {
            let red = Color::rgba(255, 0, 0, 255);
            let blue = Color::rgba(0, 0, 255, 255);
            let stops = if narrow {
                vec![
                    (0., red),
                    (0.499, red),
                    (0.499, blue),
                    (0.501, blue),
                    (0.501, red),
                    (1., red),
                ]
            } else {
                vec![(0., red), (0.5, red), (0.5, blue), (1., blue)]
            };
            b.fill_gradient(
                &Shape::rect((0., 0., 2000., 1.)),
                &GradientPaint {
                    geometry: Geometry::Linear {
                        start: (0., 0.),
                        end: (2000., 0.),
                    },
                    repeat: false,
                    stops,
                },
                Transform::identity(),
            );
        });
        for x in 995..1005 {
            pixel_near(&cpu, &ink, x, 0, 2);
        }
    }
}

#[test]
fn anisotropic_image_reduction_preserves_the_unreduced_axis() {
    if !ink_available() {
        return;
    }
    pin_font();
    let dir = std::env::temp_dir().join(format!("exact-ink-area-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pixels = Pixmap::new(256, 256).unwrap();
    for (y, row) in pixels.data_mut().chunks_exact_mut(256 * 4).enumerate() {
        for pixel in row.chunks_exact_mut(4) {
            pixel.copy_from_slice(if y % 2 == 0 {
                &[0, 0, 0, 255]
            } else {
                &[255; 4]
            });
        }
    }
    pixels.save_png(dir.join("stripe.png")).unwrap();
    let plan = contract::compile(
        "component App\n  view\n    image \"stripe.png\" width=16 height=256 object-fit=\"fill\"\n",
    )
    .unwrap()
    .encode();
    let mut frames = Vec::new();
    for painter in [PainterChoice::Cpu, PainterChoice::Ink] {
        let (mut p, _) =
            Presenter::boot_with(&plan, (), (16., 256.), 1., dir.clone(), painter).unwrap();
        p.wait_images(Duration::from_secs(3));
        frames.push(p.frame());
        assert_eq!(
            p.painter.name,
            if painter == PainterChoice::Ink {
                "ink"
            } else {
                "cpu"
            }
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    for y in 0..256 {
        pixel_near(&frames[0], &frames[1], 8, y, 2);
    }
}

#[test]
fn repeating_patterns_filter_across_tile_edges_without_mip_seams() {
    if !ink_available() {
        return;
    }
    let mut tile = Pixmap::new(8, 2).unwrap();
    for (n, p) in tile.data_mut().chunks_exact_mut(4).enumerate() {
        p.copy_from_slice(match n % 8 {
            0 => &[255, 0, 0, 255],
            7 => &[0, 0, 255, 255],
            _ => &[0, 255, 0, 255],
        });
    }
    let tile = std::sync::Arc::new(tile);
    for scale in [1., 1.25] {
        let (cpu, ink) = pair((64., 8.), |b| {
            let path = exact_kernel::svg::parse_d("M 0 0 H 64 V 8 H 0 Z");
            b.svg_path(
                &SvgPaint {
                    path: &path,
                    fill: Some(Paint::Pattern {
                        tile: tile.clone(),
                        transform: [scale, 0., 0., scale, 0.25, 0.],
                        opacity: 1.,
                    }),
                    stroke: None,
                    even_odd: false,
                    order: [0, 1, 2],
                    width: 0.,
                    cap: 0,
                    join: 0,
                    miter: 4.,
                    dash: vec![],
                    phase: 0.,
                },
                Transform::identity(),
            );
        });
        for x in 8..56 {
            pixel_near(&cpu, &ink, x, 4, 2);
        }
    }
}

#[test]
fn large_backdrop_blur_bounds_intermediate_pixels() {
    if !ink_available() {
        return;
    }
    use exact_kernel::style::{BackdropFilter, BackdropOp};
    let mut ink = exact_linux::ink::Ink::new().unwrap();
    let stats = ink.statistics();
    let mut cpu = exact_linux::raster::Raster::new();
    for b in [&mut cpu as &mut dyn Backend, &mut ink] {
        b.begin(256., 128., 1.);
        b.fill(
            &Shape::rect((0., 0., 128., 128.)),
            [20, 90, 200, 255],
            Transform::identity(),
        );
        b.backdrop_filter(
            &Shape::rect((0., 0., 256., 128.)),
            &BackdropFilter(vec![BackdropOp::Blur(50.)]),
            Transform::identity(),
        );
    }
    let a = cpu.finish().unwrap();
    let b = ink.finish().unwrap();
    let (mean, over) = band(&a, &b);
    eprintln!(
        "sigma=50, 256x128: mean {mean:.3}/255, {over:.3}% pixels beyond 32; layer bytes {}",
        stats.read().layer_bytes
    );
    assert!(mean < 5. && over < 6., "large blur: {mean}, {over}%");
    assert!(
        stats.read().layer_bytes < 4 * 256 * 128 * 4,
        "large blur retained {} bytes",
        stats.read().layer_bytes
    );
}

#[test]
fn pointer_ignores_the_content_clip() {
    if !ink_available() {
        return;
    }
    let (cpu, ink) = pair((40., 40.), |b| {
        b.push_clip(&Shape::rect((0., 0., 2., 2.)), Transform::identity());
        b.pointer(10., 10.);
        b.pop_clip();
    });
    let (mean, bad) = band(&cpu, &ink);
    assert!(mean < 5. && bad < 6., "{mean}, {bad}");
}

#[test]
fn agent_state_reports_the_current_painter_and_device() {
    for choice in painters() {
        let mut p = compiled("component App\n  view\n    text \"hello\"\n", 1., choice);
        let state: serde_json::Value =
            serde_json::from_str(&exact_linux::agent::handle(&mut p, r#"{"op":"state"}"#)).unwrap();
        let painter = &state["painter"];
        assert_eq!(painter["name"], p.painter.name);
        assert_eq!(painter["device"], serde_json::json!(p.painter.adapter));
        assert_eq!(painter["api"], serde_json::json!(p.painter.api));
        assert_eq!(painter["recovering"], false);
        assert!(painter["lastRefusal"].is_null());
    }
}
