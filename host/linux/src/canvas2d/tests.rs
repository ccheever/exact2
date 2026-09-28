use super::*;
use exact_canvas::{Context2d, Env as CanvasEnv, Images};

struct Tile;
impl Images for Tile {
    fn size(&self, _: u64, src: &str) -> Option<(u32, u32)> {
        (src == "tile").then_some((4, 4))
    }
}

fn replay(w: u32, h: u32, scale: f64, draw: impl Fn(&Context2d)) -> Pixmap {
    let ctx = Context2d::new();
    ctx.set_env(CanvasEnv {
        images: Arc::new(std::sync::Mutex::new(Some(Arc::new(Tile)))),
        ..Default::default()
    });
    draw(&ctx);
    let mut tile = Pixmap::new(4, 4).unwrap();
    tile.fill(Color::from_rgba8(0, 0, 255, 255));
    let mut images = ImageCache::new();
    images.insert("tile".into(), Arc::new(tile));
    let env = Env {
        images: &images,
        text: None,
    };
    let mut r = Replayer::new(w, h, scale, 1, 0).unwrap();
    for l in ctx.take_lists() {
        r.apply(&l, &env).unwrap();
    }
    r.pixmap
}

fn at(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn a_clockwise_arc_bulges_down_in_canvas_space() {
    // arc(50, 50, 40, 0, π): clockwise from +x through +y (down).
    let p = replay(100, 100, 1.0, |c| {
        c.set_fill_style_str("red");
        c.begin_path();
        c.arc(50.0, 50.0, 40.0, 0.0, std::f64::consts::PI).unwrap();
        c.fill();
    });
    assert_eq!(at(&p, 50, 80), [255, 0, 0, 255], "the lower half is filled");
    assert_eq!(at(&p, 50, 20)[3], 0, "the upper half is not");
}

#[test]
fn device_scale_and_author_matrix_compose() {
    let p = replay(40, 40, 2.0, |c| {
        c.translate(10.0, 0.0).unwrap();
        c.fill_rect(0.0, 0.0, 5.0, 5.0);
    });
    assert_eq!(at(&p, 21, 1)[3], 255);
    assert_eq!(at(&p, 19, 1)[3], 0);
    assert_eq!(at(&p, 29, 9)[3], 255);
    assert_eq!(at(&p, 31, 9)[3], 0);
}

#[test]
fn clear_rect_obeys_the_clip_and_the_path_survives_fill() {
    let p = replay(20, 20, 1.0, |c| {
        c.fill_rect(0.0, 0.0, 20.0, 20.0);
        c.begin_path();
        c.rect(0.0, 0.0, 10.0, 20.0);
        c.clip();
        c.clear_rect(0.0, 0.0, 20.0, 20.0);
    });
    assert_eq!(at(&p, 5, 5)[3], 0);
    assert_eq!(at(&p, 15, 5)[3], 255);
}

#[test]
fn a_clip_extent_operator_clears_outside_the_shape_within_the_clip() {
    let p = replay(20, 20, 1.0, |c| {
        c.fill_rect(0.0, 0.0, 20.0, 20.0);
        c.begin_path();
        c.rect(0.0, 0.0, 10.0, 20.0);
        c.clip();
        c.set_global_composite_operation("copy").unwrap();
        c.set_fill_style_str("red");
        c.fill_rect(0.0, 0.0, 5.0, 5.0);
    });
    assert_eq!(at(&p, 2, 2), [255, 0, 0, 255], "the shape");
    assert_eq!(at(&p, 7, 12)[3], 0, "outside the shape, inside the clip");
    assert_eq!(at(&p, 15, 12)[3], 255, "outside the clip");
}

#[test]
fn a_shadow_is_offset_in_canvas_pixels_whatever_the_matrix() {
    let p = replay(40, 40, 1.0, |c| {
        c.set_shadow_color("black");
        c.set_shadow_offset_x(10.0);
        c.scale(3.0, 3.0).unwrap();
        c.fill_rect(0.0, 0.0, 3.0, 3.0);
    });
    assert_eq!(at(&p, 4, 4)[3], 255, "the shape");
    assert_eq!(at(&p, 16, 4)[3], 255, "its shadow, 10 px right");
    assert_eq!(at(&p, 30, 4)[3], 0, "not 30 px right");
}

#[test]
fn images_patterns_and_pixels_replay() {
    let p = replay(30, 30, 1.0, |c| {
        c.draw_image_with_image_handle_and_dw_and_dh("tile", 0.0, 0.0, 8.0, 8.0)
            .unwrap();
        let pat = c
            .create_pattern_with_image_handle("tile", "repeat-x")
            .unwrap()
            .unwrap();
        c.set_fill_style_canvas_pattern(&pat);
        c.fill_rect(0.0, 0.0, 26.0, 20.0);
        let mut d = c.create_image_data_with_sw_and_sh(2.0, 2.0).unwrap();
        d.data.copy_from_slice(&[255, 0, 0, 255].repeat(4));
        c.put_image_data(&d, 28.0, 0.0).unwrap();
    });
    assert_eq!(at(&p, 6, 6), [0, 0, 255, 255], "drawImage scaled");
    assert_eq!(at(&p, 25, 2), [0, 0, 255, 255], "repeat-x across");
    assert_eq!(at(&p, 25, 10)[3], 0, "but not down");
    assert_eq!(at(&p, 29, 1), [255, 0, 0, 255], "putImageData");
}

#[test]
fn a_conic_gradient_starts_at_its_angle_and_turns_clockwise() {
    let p = replay(40, 40, 1.0, |c| {
        let g = c.create_conic_gradient(0.0, 20.0, 20.0).unwrap();
        g.add_color_stop_f64(0.0, "red").unwrap();
        g.add_color_stop_f64(0.5, "blue").unwrap();
        g.add_color_stop_f64(1.0, "red").unwrap();
        c.set_fill_style_canvas_gradient(&g);
        c.fill_rect(0.0, 0.0, 40.0, 40.0);
    });
    assert!(at(&p, 38, 20)[0] > 200, "red at angle 0");
    assert!(at(&p, 2, 20)[2] > 200, "blue half way round");
}
