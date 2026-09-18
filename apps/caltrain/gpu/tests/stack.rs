//! The card stack (LLP 1014 D5), headless: cards bound, given frames, and
//! rendered over a moving clock; their placements — the homographies the
//! host hit-tests through — are where the fan says they are, nearest first
//! where they overlap, settled once the springs are, and a card whose train
//! has left is nowhere a tap can reach.

use caltrain_gpu::StackSurface;
use exact_gpu::fixture;
use exact_gpu::{Frame, Surface, Value};

fn departure(id: &str, at: f64) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::Number(101.0),
        Value::str("Local"),
        Value::str("San Francisco"),
        Value::Number(at),
    ])
}

fn board() -> Value {
    Value::List(
        vec![
            departure("gone", -1.0),
            departure("a", 600_000.0),
            departure("b", 1_200_000.0),
            departure("c", 1_800_000.0),
        ]
        .into(),
    )
}

fn frame(now_ms: f64) -> Frame {
    Frame {
        width: 380.0,
        height: 460.0,
        scale: 1.0,
        now_ms,
        children_generation: 1,
        seekable: false,
        shader_generation: exact_gpu::shaders::shader_generation(),
    }
}

/// A homography applied to a point.
fn map(h: &[f32; 9], x: f32, y: f32) -> (f32, f32) {
    let w = h[6] * x + h[7] * y + h[8];
    (
        (h[0] * x + h[1] * y + h[2]) / w,
        (h[3] * x + h[4] * y + h[5]) / w,
    )
}

#[test]
fn the_fan_places_cards_down_the_canvas_and_settles() {
    let Ok(gpu) = fixture::device() else {
        eprintln!("no adapter; the stack fixture is skipped");
        return;
    };
    // The shaders travel as files (LLP 1030 D8): registered as a host would.
    exact_gpu::shaders::load_dir(&caltrain_gpu::shader_dir(), &caltrain_gpu::REGISTRY).unwrap();
    let mut stack = StackSurface::new();
    stack
        .bind(
            &[
                board(),
                Value::Option(None),
                Value::Bool(true),
                Value::Number(0.0),
            ],
            None,
        )
        .unwrap();
    assert!(stack.wants_children_each());
    // Four cards, 90 points tall, laid out by the kernel as a column.
    let texture = gpu
        .device
        .create_texture(&exact_gpu::wgpu::TextureDescriptor {
            label: Some("card"),
            size: exact_gpu::wgpu::Extent3d {
                width: 380,
                height: 90,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: exact_gpu::wgpu::TextureDimension::D2,
            format: exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
            usage: exact_gpu::wgpu::TextureUsages::TEXTURE_BINDING
                | exact_gpu::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &vec![255u8; 380 * 90 * 4],
        exact_gpu::wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(380 * 4),
            rows_per_image: None,
        },
        exact_gpu::wgpu::Extent3d {
            width: 380,
            height: 90,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    for i in 0..4 {
        stack.child(i, Some(&view), [0.0, 90.0 * i as f32, 380.0, 90.0]);
    }
    stack.children_count(4);

    // The first frame: cards start on the closed deck and want to move.
    let (_, wants) = fixture::render(&gpu, &mut stack, &frame(0.0)).unwrap();
    assert!(wants, "the springs are running");
    // Two seconds later they have settled into the fan.
    let (px, wants) = fixture::render(&gpu, &mut stack, &frame(2000.0)).unwrap();
    assert!(!wants, "settled");
    px.save("stack");
    let a = stack.placement(1).unwrap();
    let b = stack.placement(2).unwrap();
    let gone = stack.placement(0).unwrap();
    // The first future card is nearest and lowest; the next one sits above
    // it, about half a card higher and a little smaller; the departed one is
    // off the canvas.
    let (ax, ay) = map(&a.homography, 0.0, 0.0);
    let (_, by) = map(&b.homography, 0.0, 0.0);
    assert!(ax.abs() < 8.0 && ay > 200.0, "card a at {ax}, {ay}");
    assert!(
        by < ay - 30.0 && by > ay - 80.0,
        "card b at {by}, a at {ay}"
    );
    let (gx, _) = map(&gone.homography, 0.0, 0.0);
    assert!(gx > 10_000.0, "a departed card is nowhere: {gx}");
    // Nearer cards come first: a is nearer than b.
    assert!(a.depth > b.depth, "a {} b {}", a.depth, b.depth);
    // Something was drawn: white cards over a transparent canvas.
    let drawn = px.count(|p| p[3] > 200);
    assert!(drawn > 380 * 90, "{drawn} opaque pixels");
    // A frame with no clock movement neither moves nor wants.
    let (_, wants) = fixture::render(&gpu, &mut stack, &frame(2000.0)).unwrap();
    assert!(!wants, "still settled");

    // Focus the second card: it comes forward, and wants frames again.
    stack
        .bind(
            &[
                board(),
                Value::Option(Some(Value::str("b").into())),
                Value::Bool(true),
                Value::Number(0.0),
            ],
            None,
        )
        .unwrap();
    let (_, wants) = fixture::render(&gpu, &mut stack, &frame(2000.0)).unwrap();
    assert!(wants, "the focus moves");
    let (_, _) = fixture::render(&gpu, &mut stack, &frame(4000.0)).unwrap();
    let b2 = stack.placement(2).unwrap();
    assert!(
        b2.depth > a.depth,
        "the focused card is nearest: {} vs {}",
        b2.depth,
        a.depth
    );

    // The agent's clock jumps: `clock +60000` is one frame sixty seconds on.
    // The springs integrate at most two seconds of it in 1/120 s steps and
    // arrive settled — the same poses a display link would have reached —
    // and every number stays finite.
    stack
        .bind(
            &[
                board(),
                Value::Option(None),
                Value::Bool(false),
                Value::Number(0.0),
            ],
            None,
        )
        .unwrap();
    let (_, wants) = fixture::render(&gpu, &mut stack, &frame(64_000.0)).unwrap();
    assert!(!wants, "a sixty-second jump lands settled");
    let closed = stack.placement(1).unwrap();
    let (cx, cy) = map(&closed.homography, 0.0, 0.0);
    assert!(cx.is_finite() && cy.is_finite(), "finite: {cx}, {cy}");
    assert!(
        cx.abs() < 8.0 && cy < 40.0,
        "the deck closed: card a back at the top ({cx}, {cy})"
    );
}
