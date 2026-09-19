#![cfg(not(target_arch = "wasm32"))]
use exact_game::{Actions, Camera, Data, Game, Input, Material, Mesh, Transform, Vec3, World};
use exact_game_render::WorldSurface;
use exact_gpu::{
    fixture::{self, Pixels},
    Frame, Gpu, InputEvent, Surface, Value,
};
use greybox_logic::Greybox;

fn gpu() -> Option<Gpu> {
    match fixture::device() {
        Ok(gpu) => {
            eprintln!("WorldSurface GPU: {:?}", gpu.adapter.get_info());
            Some(gpu)
        }
        Err(reason) => {
            eprintln!("SKIP WorldSurface GPU proof: {reason}");
            None
        }
    }
}
fn frame(now_ms: f64) -> Frame {
    Frame {
        width: 640.,
        height: 360.,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.0,
        children_generation: 0,
        shader_generation: 0,
    }
}
fn render<G: Game>(gpu: &Gpu, surface: &mut WorldSurface<G>, now: f64, name: &str) -> Pixels {
    let (pixels, _) = fixture::render(gpu, surface, &frame(now)).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    pixels.save(name);
    pixels
}
fn key(surface: &mut dyn Surface, code: &str, down: bool, at_ms: f64) {
    surface.input(&InputEvent::Key {
        code: code.into(),
        key: String::new(),
        down,
        repeat: false,
        at_ms,
    });
}
#[derive(Default, Data, Debug)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
#[derive(Default, Data)]
struct LayoutEntity {
    screen: Rect,
}
#[derive(Default, Data)]
struct Layout {
    entity: LayoutEntity,
}
fn bounds(surface: &mut dyn Surface, entity: &str) -> Rect {
    let reply = surface
        .agent(&format!(
            r#"{{"op":"layout","entity":"{entity}","width":640,"height":360}}"#
        ))
        .unwrap();
    exact_game::json::from_str::<Layout>(&reply)
        .unwrap()
        .entity
        .screen
}
fn patch<'a>(image: &'a Pixels, rect: &Rect) -> impl Iterator<Item = (u32, u32, [u8; 4])> + 'a {
    let x0 = (rect.x.floor() as i32).clamp(0, image.width as i32) as u32;
    let y0 = (rect.y.floor() as i32).clamp(0, image.height as i32) as u32;
    let x1 = ((rect.x + rect.w).ceil() as i32).clamp(0, image.width as i32) as u32;
    let y1 = ((rect.y + rect.h).ceil() as i32).clamp(0, image.height as i32) as u32;
    (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y, image.at(x, y))))
}
fn orange(p: [u8; 4]) -> bool {
    p[0] as i32 - p[1] as i32 > 15 && p[1] as i32 - p[2] as i32 > 15
}
fn orange_y(image: &Pixels, rect: &Rect) -> f64 {
    let mut total = 0.;
    let mut count = 0;
    for (_, y, p) in patch(image, rect).filter(|(_, _, p)| orange(*p)) {
        total += y as f64;
        count += 1;
        let _ = p;
    }
    assert!(count > 30, "no capsule pixels in {rect:?}");
    total / count as f64
}
fn luminance(image: &Pixels, rect: &Rect) -> f64 {
    let mut total = 0.;
    let mut count = 0;
    for (_, _, p) in patch(image, rect) {
        total += 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        count += 1;
    }
    assert!(count > 0);
    total / count as f64
}
#[test]
fn greybox_surface_agent_pixels_and_beacon() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut surface = WorldSurface::<Greybox>::default();
    surface
        .bind(
            &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
            None,
        )
        .unwrap();
    assert!(surface.wants_input());
    assert!(surface.published().unwrap().contains(r#""beacons":0"#));
    assert!(surface.published().is_none());
    assert!(surface.messages().is_empty());
    let first = render(&gpu, &mut surface, 0., "world-greybox-0");
    let rect = bounds(&mut surface, "player");
    let y = orange_y(&first, &rect);
    assert!((rect.y as f64..(rect.y + rect.h) as f64).contains(&y));
    let plane = first.at((rect.x - 8.).max(0.) as u32, (rect.y + rect.h * 0.6) as u32);
    assert!(!orange(plane), "plane probe accidentally hits the capsule");
    assert!(first.count(|p| p[0] > 20 || p[1] > 20 || p[2] > 20) > 200_000);
    render(&gpu, &mut surface, 350., "world-greybox-350");
    key(&mut surface, "KeyW", true, 350.);
    // 1.35 s of W under Move's acceleration and braking stops about 1.3 m short of the
    // beacon at z = -6.5: in range, and not hiding it behind the capsule.
    render(&gpu, &mut surface, 1700., "world-greybox-walk");
    key(&mut surface, "KeyW", false, 1700.);
    let before = render(&gpu, &mut surface, 2150., "world-beacon-before");
    let rect = bounds(&mut surface, "beacon-1");
    key(&mut surface, "KeyE", true, 2150.);
    key(&mut surface, "KeyE", false, 2180.);
    let after = render(&gpu, &mut surface, 2850., "world-beacon-after");
    let before_luma = luminance(&before, &rect);
    let after_luma = luminance(&after, &bounds(&mut surface, "beacon-1"));
    eprintln!("beacon patch luminance {before_luma:.2} -> {after_luma:.2}");
    assert!(after_luma > before_luma + 8.);
    surface
        .bind(
            &[Value::Number(7.), Value::Bool(true), Value::Bool(false)],
            None,
        )
        .unwrap();
    let (_, wants) = fixture::render(&gpu, &mut surface, &frame(2810.)).unwrap();
    assert!(
        !wants,
        "a paused unchanged world should stop requesting frames"
    );
    let state = surface.agent(r#"{"op":"state"}"#).unwrap();
    assert!(state.contains(r#""wallClock":true"#));
    assert!(state.contains(r#""frameMs":{"p50":0"#));
    assert!(surface.published().unwrap().contains(r#""beacons":1"#));
}
// Greybox follows the capsule exactly. Freeze ONLY its camera in this fixture so
// screen-space movement proves the input → simulation → slot upload path.
struct FixedCameraGreybox;
impl Game for FixedCameraGreybox {
    const ID: &'static str = "fixed-camera-greybox";
    type Args = <Greybox as Game>::Args;
    fn setup(w: &mut World, a: &Self::Args) {
        Greybox::setup(w, a)
    }

    fn actions() -> Actions {
        Greybox::actions()
    }
    fn tick(w: &mut World, i: &Input, args: &Self::Args) {
        Greybox::tick(w, i, args);
        let e = w.named("camera").unwrap();
        *w.get_mut::<Transform>(e).unwrap() =
            Transform::at(0., 5.9, 8.).looking_at(Vec3::new(0., 0.9, 0.), Vec3::Y);
    }
}
#[test]
fn holding_w_moves_capsule_pixels_up_screen() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = WorldSurface::<FixedCameraGreybox>::default();
    s.bind(
        &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
        None,
    )
    .unwrap();
    let before = render(&gpu, &mut s, 0., "world-player-before");
    let by = orange_y(&before, &bounds(&mut s, "player"));
    key(&mut s, "KeyW", true, 0.);
    let after = render(&gpu, &mut s, 1000., "world-player-after");
    let ay = orange_y(&after, &bounds(&mut s, "player"));
    eprintln!("capsule pixel centroid y {by:.2} -> {ay:.2}");
    assert!(ay + 25. < by);
}
struct Stop;
impl Game for Stop {
    type Args = ();
    const ID: &'static str = "stop-pixel-proof";
    fn setup(w: &mut World, _: &Self::Args) {
        w.spawn((
            Transform::default(),
            Mesh::cube(1.0),
            Material::rgb(0.8, 0.2, 0.1),
        ));
        w.spawn((Transform::at(0., 0., 8.), Camera::default()));
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        if w.tick() == 0 {
            for (_, (_, t)) in w.query::<(&Mesh, &mut Transform)>().iter() {
                t.position.x += 2.;
            }
        }
    }
}
#[test]
fn stopped_pixels_identical_across_alphas_and_agent_seeks_feed_history() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut s = WorldSurface::<Stop>::default();
    s.bind(&[], None).unwrap();
    let initial = render(&gpu, &mut s, 0., "world-stop-initial");
    render(&gpu, &mut s, 17., "world-stop-moving");
    render(&gpu, &mut s, 34., "world-stop-still-one");
    let settled = render(&gpu, &mut s, 51., "world-stop-still-two");
    assert_ne!(initial.data, settled.data);
    for now in [52., 57., 63., 65.] {
        assert_eq!(
            settled.data,
            render(&gpu, &mut s, now, "world-stop-alpha").data
        );
    }
    let mut via_agent = WorldSurface::<FixedCameraGreybox>::default();
    via_agent
        .bind(
            &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
            None,
        )
        .unwrap();
    let mut via_frame = WorldSurface::<FixedCameraGreybox>::default();
    via_frame
        .bind(
            &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
            None,
        )
        .unwrap();
    render(&gpu, &mut via_agent, 0., "world-agent-start");
    render(&gpu, &mut via_frame, 0., "world-frame-start");
    key(&mut via_agent, "KeyW", true, 0.);
    key(&mut via_frame, "KeyW", true, 0.);
    via_agent.agent(r#"{"op":"clock","now":1005}"#).unwrap();
    assert_eq!(
        render(&gpu, &mut via_agent, 1005., "world-agent-seek").data,
        render(&gpu, &mut via_frame, 1005., "world-frame-seek").data
    );
}
#[test]
fn bind_refusal_pause_messages_and_perf_do_not_need_a_device() {
    let mut surface = WorldSurface::<Greybox>::default();
    assert!(surface.bind(&[], None).unwrap_err().0.contains("seed"));
    surface
        .bind(
            &[Value::Number(7.), Value::Bool(false), Value::Bool(false)],
            None,
        )
        .unwrap();
    let hash = surface.sim().unwrap().world().hash();
    let state = surface.agent(r#"{"op":"state"}"#).unwrap();
    assert!(state.contains("perf"));
    assert_eq!(hash, surface.sim().unwrap().world().hash());
    assert!(!surface
        .agent(r#"{"op":"state","entity":"player"}"#)
        .unwrap()
        .contains("perf"));
    assert!(!surface
        .agent(r#"{"op":"tree","summary":true}"#)
        .unwrap()
        .contains("perf"));
    let gen = surface.sim().unwrap().generation();
    surface
        .bind(
            &[Value::Number(8.), Value::Bool(true), Value::Bool(false)],
            None,
        )
        .unwrap();
    assert_ne!(gen, surface.sim().unwrap().generation());
    surface.agent(r#"{"op":"clock","now":0}"#);
    surface.agent(r#"{"op":"clock","now":1000}"#);
    assert_eq!(surface.sim().unwrap().world().tick(), 0);
}

#[test]
fn beacons_grid_fog_and_bloom() {
    use beacons_logic::{Beacons, Options};
    struct Atmosphere;
    impl Game for Atmosphere {
        type Args = Options;
        const ID: &'static str = Beacons::ID;
        fn actions() -> Actions {
            Beacons::actions()
        }
        fn setup(w: &mut World, args: &Options) {
            Beacons::setup(w, args);
            w.insert_resource(exact_game::Environment::default());
        }
        fn tick(w: &mut World, input: &Input, args: &Options) {
            Beacons::tick(w, input, args);
        }
    }
    let Some(gpu) = gpu() else { return };
    let mut sim = exact_game::Sim::<Atmosphere>::new(Options {
        seed: 7,
        paused: false,
        restart: false,
    })
    .unwrap();
    // The r5 consumer leaves its ground unnamed. Identify its plane structurally.
    let ground = sim
        .world()
        .query::<&Mesh>()
        .iter()
        .find_map(|(e, mesh)| matches!(mesh, Mesh::Plane { .. }).then_some(e))
        .expect("Beacons ground plane");
    // This render fixture opts into a grid; the consumer's ground is plain.
    sim.world()
        .get_mut::<Material>(ground)
        .unwrap()
        .grid_spacing = 1.0;
    let player = sim.world().named("player").unwrap();
    sim.world_mut()
        .teleport(player, Transform::at(8.0, 0.9, 0.0));
    sim.tap("KeyE");
    sim.run(1000.0);
    let mut surface = WorldSurface::<Atmosphere>::default();
    surface
        .bind(
            &[Value::Number(7.0), Value::Bool(false), Value::Bool(false)],
            None,
        )
        .unwrap();
    surface
        .restore(&sim.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    let pixels = render(&gpu, &mut surface, 0.0, "beacons-defaults");
    assert!(pixels.data.chunks_exact(4).any(|p| p[0] > 200));
    assert_eq!(
        surface
            .sim()
            .unwrap()
            .world()
            .get::<Material>(ground)
            .unwrap()
            .grid_spacing,
        1.0
    );
    sim.world_mut()
        .get_mut::<Material>(ground)
        .unwrap()
        .grid_spacing = 0.0;
    surface
        .restore(&sim.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    let plain = render(&gpu, &mut surface, 0.0, "beacons-without-grid");
    let changed = pixels
        .data
        .chunks_exact(4)
        .zip(plain.data.chunks_exact(4))
        .filter(|(a, b)| (i16::from(a[0]) - i16::from(b[0])).abs() > 3)
        .count();
    assert!(
        changed > 1000,
        "grid must add visible detail: {changed} pixels"
    );
    // At a distance, unresolved lines retain their mean instead of disappearing.
    sim.world_mut()
        .get_mut::<Material>(ground)
        .unwrap()
        .grid_spacing = 0.001;
    surface
        .restore(&sim.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    let distant = render(&gpu, &mut surface, 0.0, "beacons-distant-grid");
    let brighter = distant
        .data
        .chunks_exact(4)
        .zip(plain.data.chunks_exact(4))
        .filter(|(a, b)| i16::from(a[0]) > i16::from(b[0]) + 3)
        .count();
    assert!(
        brighter > 1000,
        "unresolved grid retains its lined mean: {brighter}"
    );
    sim.world_mut()
        .get_mut::<Material>(ground)
        .unwrap()
        .grid_spacing = 0.0;
    sim.world_mut().get_mut::<Material>(ground).unwrap().color[3] = -2.0;
    surface
        .restore(&sim.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    assert!(
        plain.data == render(&gpu, &mut surface, 0.0, "beacons-negative-alpha").data,
        "negative alpha must not enable a grid"
    );

    let env = exact_game::Environment {
        fog: None,
        ..Default::default()
    };
    sim.world_mut().insert_resource(env);
    surface
        .restore(&sim.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    let clear = render(&gpu, &mut surface, 0.0, "beacons-no-fog");
    assert!(
        plain
            .data
            .chunks_exact(4)
            .zip(clear.data.chunks_exact(4))
            .filter(|(a, b)| (i16::from(a[0]) - i16::from(b[0])).abs() > 3)
            .count()
            > 1000
    );
    let env = exact_game::Environment {
        bloom: None,
        ..Default::default()
    };
    sim.world_mut().insert_resource(env);
    surface
        .restore(&sim.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    let dark = render(&gpu, &mut surface, 0.0, "beacons-no-bloom");
    assert!(
        plain
            .data
            .chunks_exact(4)
            .zip(dark.data.chunks_exact(4))
            .filter(|(a, b)| (i16::from(a[0]) - i16::from(b[0])).abs() > 1)
            .count()
            > 50
    );
}
