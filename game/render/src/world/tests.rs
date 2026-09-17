use super::*;
use exact_game::{
    Args, Camera, Clock, DirectionalLight, Entity, Game, Input, PointLight, Quat, Sim, Vec3,
};

#[derive(Debug, PartialEq)]
enum Call {
    Begin(Rewrite),
    Transform(u32, usize, bool),
    Material(u32, usize),
    Batches,
}
struct Recording {
    calls: Vec<Call>,
    current: Vec<f32>,
    previous: Vec<f32>,
    materials: Vec<f32>,
    slots: Vec<u32>,
    batches: Vec<Batch>,
    meshes: Vec<(Vec<Vertex>, Vec<u32>)>,
    limit: u32,
    record: bool,
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            current: Vec::new(),
            previous: Vec::new(),
            materials: Vec::new(),
            slots: Vec::new(),
            batches: Vec::new(),
            meshes: Vec::new(),
            limit: 1_000_000,
            record: true,
        }
    }
}
impl Recording {
    fn call(&mut self, call: Call) {
        if self.record {
            self.calls.push(call);
        }
    }
    fn position(&self, e: Entity, previous: bool) -> Vec3 {
        let v = if previous {
            &self.previous
        } else {
            &self.current
        };
        Vec3::from_slice(&v[e.index() as usize * 10..][..3])
    }
}
impl Writes for Recording {
    fn max_slots(&self) -> u32 {
        self.limit
    }
    fn begin_tick(&mut self, rewrite: Rewrite) {
        self.call(Call::Begin(rewrite));
        std::mem::swap(&mut self.current, &mut self.previous);
        if rewrite == Rewrite::Some {
            self.current.clone_from(&self.previous);
        }
    }
    fn transforms(&mut self, first: u32, values: &[f32], both: bool) -> Result<(), RenderError> {
        self.call(Call::Transform(first, values.len(), both));
        let start = first as usize * 10;
        let end = start + values.len();
        if end > self.current.len() {
            self.current.resize(end, 0.0);
        }
        self.current[start..end].copy_from_slice(values);
        if both {
            if end > self.previous.len() {
                self.previous.resize(end, 0.0);
            }
            self.previous[start..end].copy_from_slice(values);
        }
        Ok(())
    }
    fn materials(&mut self, first: u32, values: &[f32]) -> Result<(), RenderError> {
        self.call(Call::Material(first, values.len()));
        let start = first as usize * 12;
        let end = start + values.len();
        if end > self.materials.len() {
            self.materials.resize(end, 0.0);
        }
        self.materials[start..end].copy_from_slice(values);
        Ok(())
    }
    fn mesh(&mut self, v: &[Vertex], i: &[u32]) -> MeshId {
        let id = MeshId(self.meshes.len());
        self.meshes.push((v.to_vec(), i.to_vec()));
        id
    }
    fn batches(&mut self, b: &[Batch], s: &[u32]) -> Result<(), RenderError> {
        self.call(Call::Batches);
        self.batches.clear();
        self.batches.extend_from_slice(b);
        self.slots.clear();
        self.slots.extend_from_slice(s);
        Ok(())
    }
}
struct Moving;
impl Game for Moving {
    const ID: &'static str = "feed-test";
    fn setup(w: &mut World, _: &Args) -> Result<(), String> {
        w.spawn((Transform::default(), Mesh::Cube, Material::default()));
        Ok(())
    }
    fn tick(w: &mut World, _: &Input) {
        for (_, t) in w.query::<&mut Transform>().iter() {
            t.position.x += 1.0;
        }
    }
}
struct Stop;
impl Game for Stop {
    const ID: &'static str = "feed-stop";
    fn setup(w: &mut World, a: &Args) -> Result<(), String> {
        Moving::setup(w, a)
    }
    fn tick(w: &mut World, i: &Input) {
        if w.tick() == 0 {
            Moving::tick(w, i);
        }
    }
}
#[test]
fn pages_are_whole_ordered_and_holes_are_zero() {
    let mut w = World::new(60, 0);
    for i in 0..PAGE * 3 {
        let e = w.spawn(());
        if i == 7 || i == 2 * PAGE + 10 {
            w.insert(e, Transform::at(i as f32, 1., 2.));
        }
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let writes: Vec<_> = r
        .calls
        .iter()
        .filter(|c| matches!(c, Call::Transform(..)))
        .collect();
    assert_eq!(
        writes,
        [
            &Call::Transform(0, PAGE * 10, true),
            &Call::Transform((PAGE * 2) as u32, PAGE * 10, true)
        ]
    );
    assert!(r.current[..70].iter().all(|v| *v == 0.0));
    r.calls.clear();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.is_empty());
}
#[test]
fn moved_then_two_still_ticks_stop_all_history_work() {
    let mut sim = Sim::<Stop>::new(&[]).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    r.calls.clear();
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert!(r.calls.contains(&Call::Begin(Rewrite::All)));
    assert_ne!(r.previous, r.current); // Includes the setup-to-first-tick stamp collision.
    assert!(!r.calls.contains(&Call::Batches));
    r.calls.clear();
    sim.advance_with(34., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert_eq!(r.calls, [Call::Begin(Rewrite::Some)]);
    assert_eq!(r.previous, r.current);
    r.calls.clear();
    sim.advance_with(51., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert!(r.calls.is_empty());
    assert_eq!(r.previous, r.current);
}
#[test]
fn propagated_chain_overlays_page_and_fresh_teleport_writes_both() {
    let mut w = World::new(60, 0);
    let root = w.spawn(Transform {
        rotation: Quat::from_rotation_y(0.7),
        ..Transform::at(2., 3., 4.).with_scale(2.)
    });
    let a = w.spawn((Transform::at(1., 0., 0.).with_scale(3.), Parent(root)));
    let b = w.spawn((Transform::at(0., 2., 0.), Parent(a), Mesh::Cube));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let expected = Vec3::new(2., 3., 4.) + Quat::from_rotation_y(0.7) * Vec3::new(2., 12., 0.);
    assert!(r.position(b, false).distance(expected) < 1e-5);
    assert!(r.current[b.index() as usize * 10 + 7..][..3]
        .iter()
        .all(|s| (*s - 6.).abs() < 1e-5));
    r.calls.clear();
    w.teleport(b, Transform::at(10., 0., 0.));
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Transform(b.index(), 10, true)));
    assert_eq!(r.position(b, true), r.position(b, false));
    let e = w.spawn((Transform::at(50., 0., 0.), Mesh::Cube));
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.position(e, true), Vec3::new(50., 0., 0.));
}
#[test]
fn structure_and_visibility_rebuild_but_movement_does_not() {
    let mut w = World::new(60, 0);
    let a = w.spawn((Transform::default(), Mesh::Cube));
    let b = w.spawn((Transform::default(), Mesh::Sphere, Visible(false)));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.slots, [a.index()]);
    r.calls.clear();
    w.get_mut::<Transform>(a).unwrap().position.x = 1.;
    f.feed_to(&w, &mut r).unwrap();
    assert!(!r.calls.contains(&Call::Batches));
    w.insert(b, Visible(true));
    w.despawn(a);
    let c = w.spawn((Transform::at(7., 0., 0.), Mesh::Cube));
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&c.index()) && r.slots.contains(&b.index()));
    assert_eq!(r.position(c, true).x, 7.);
    w.despawn(c);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.slots, [b.index()]);
    w.remove::<Transform>(b);
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.is_empty());
}
#[test]
fn materials_repack_pages_only_on_revision_and_default_missing_values() {
    let mut w = World::new(60, 0);
    let a = w.spawn((
        Transform::default(),
        Material::rgb(0.2, 0.3, 0.4).emissive(2., 3., 4.),
    ));
    let b = w.spawn((Transform::default(), Mesh::Cube));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Material(0, PAGE * 12)));
    assert_eq!(&r.materials[..9], &[0.2, 0.3, 0.4, 1., 0., 0.5, 2., 3., 4.]);
    assert_eq!(
        &r.materials[b.index() as usize * 12..][..12],
        &material_floats(Material::default())
    );
    r.calls.clear();
    w.get_mut::<Transform>(a).unwrap().position.x = 2.;
    f.feed_to(&w, &mut r).unwrap();
    assert!(!r.calls.iter().any(|c| matches!(c, Call::Material(..))));
    w.remove::<Material>(a);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(&r.materials[..12], &material_floats(Material::default()));
}
#[test]
fn long_advance_feeds_only_last_two_ticks() {
    let mut sim = Sim::<Moving>::new(&[]).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    let mut ticks = Vec::new();
    sim.advance_with(60_000., Clock::Seekable, |w, left| {
        if left < 2 {
            ticks.push(w.tick());
            f.feed_to(w, &mut r).unwrap();
        }
    });
    assert_eq!(ticks, [3599, 3600]);
    assert_eq!(r.previous[0], 3599.);
    assert_eq!(r.current[0], 3600.);
}
#[test]
fn capacity_refuses_before_history_and_allows_partial_last_page() {
    let mut w = World::new(60, 0);
    for _ in 0..=PAGE {
        w.spawn(Transform::default());
    }
    let mut f = Feed::default();
    let mut r = Recording {
        limit: PAGE as u32,
        ..Default::default()
    };
    let e = f.feed_to(&w, &mut r).unwrap_err();
    assert_eq!(e.arena, "transforms");
    assert_eq!(e.slot, PAGE as u64);
    assert!(r.calls.is_empty());
    r.limit += 1;
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Transform(PAGE as u32, 10, true)));
}
#[test]
fn primitive_dimensions_and_bit_keys_match_agent_geometry() {
    let mut w = World::new(60, 0);
    for mesh in [
        Mesh::Sphere,
        Mesh::Cylinder,
        Mesh::Plane { size: 40. },
        Mesh::Capsule {
            radius: 0.4,
            height: 1.,
        },
        Mesh::Capsule {
            radius: 0.4,
            height: 1.,
        },
        Mesh::Capsule {
            radius: 0.5,
            height: 1.,
        },
        Mesh::Asset("future".into()),
        Mesh::Cube,
    ] {
        w.spawn((Transform::default(), mesh));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.meshes.len(), 6);
    for (i, expected) in [
        (0, Vec3::ONE),
        (1, Vec3::new(1., 0.5, 1.)),
        (2, Vec3::new(20., 0., 20.)),
        (3, Vec3::new(0.4, 0.9, 0.4)),
    ] {
        let extent = r.meshes[i]
            .0
            .iter()
            .map(|v| Vec3::from_array(v.position).abs())
            .fold(Vec3::ZERO, Vec3::max);
        assert!(
            extent.distance(expected) < 1e-5,
            "{extent:?} != {expected:?}"
        );
    }
}
#[test]
fn camera_slerps_and_nearest_lights_interpolate_without_frame_scans() {
    let mut sim = Sim::<Moving>::new(&[]).unwrap();
    let w = sim.world_mut();
    let camera = w.spawn((Transform::default(), Camera::default()));
    w.spawn((Transform::default(), DirectionalLight::default()));
    let mut lights = Vec::new();
    for i in 0..20 {
        lights.push(w.spawn((Transform::at(i as f32 + 1., 0., 0.), PointLight::default())));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    // Every pose advanced +X by one; even the one camera and selected lights blend.
    let input = f.frame(sim.world(), 0.25, 2.);
    assert_eq!(input.camera_position.x, 0.25);
    assert_eq!(input.points.len(), 16);
    assert_eq!(input.points[0].position.x, 1.25);
    assert_eq!(input.points[15].position.x, 16.25);
    assert!(input.sun.unwrap().shadows.is_some());
    assert!(input.bloom.is_some());
    assert!(input.environment.fog.is_none());
    sim.world().get_mut::<Transform>(camera).unwrap().rotation = Quat::from_rotation_y(1.0);
    f.feed_to(sim.world(), &mut r).unwrap();
    let input = f.frame(sim.world(), 0.5, 2.);
    let forward = input.view.inverse().transform_vector3(-Vec3::Z);
    assert!(forward.distance(Quat::from_rotation_y(0.5) * -Vec3::Z) < 1e-5);
    sim.world_mut()
        .teleport(lights[19], Transform::at(0.9, 0., 0.));
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(
        f.frame(sim.world(), 0.5, 2.).points[0].position,
        Vec3::new(0.9, 0., 0.)
    );
}

#[allow(unsafe_code)]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
    struct Counter;
    #[global_allocator]
    static ALLOCATOR: Counter = Counter;
    // SAFETY: all allocation operations delegate unchanged to the system allocator.
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            COUNT.with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            COUNT.with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.realloc(ptr, layout, size) }
        }
    }
    pub fn count(f: impl FnOnce()) -> usize {
        COUNT.with(|c| c.set(Some(0)));
        f();
        COUNT.with(|c| c.replace(None).unwrap())
    }
}
#[test]
fn steady_sim_feed_and_frame_inputs_allocate_nothing() {
    let mut sim = Sim::<Moving>::new(&[]).unwrap();
    let camera = sim
        .world_mut()
        .spawn((Transform::at(0., 0., 10.), Camera::default()));
    sim.world_mut()
        .spawn((Transform::at(1., 1., 1.), PointLight::default()));
    let mut f = Feed::default();
    let mut r = Recording {
        record: false,
        ..Default::default()
    };
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    let count = allocations::count(|| {
        for i in 2..240 {
            sim.advance_with(
                i as f64 * 1000. / 60. + 0.001,
                Clock::Seekable,
                |w, left| {
                    if left < 2 {
                        f.feed_to(w, &mut r).unwrap();
                    }
                },
            );
            let frame = f.frame(sim.world(), 0.5, 16. / 9.);
            std::hint::black_box(frame.camera_position);
        }
    });
    assert_eq!(count, 0);
    assert!(sim.world().contains(camera));
}

#[test]
fn first_transform_on_an_older_entity_initializes_history_and_reset_reuses_meshes() {
    struct Later;
    impl Game for Later {
        const ID: &'static str = "late-pose";
        fn setup(w: &mut World, _: &Args) -> Result<(), String> {
            w.spawn(Mesh::Cube);
            Ok(())
        }
        fn tick(w: &mut World, _: &Input) {
            let e = w.query::<&Mesh>().iter().next().unwrap().0;
            if !w.has::<Transform>(e) {
                w.insert(e, Transform::at(9., 0., 0.));
            }
        }
    }
    let mut sim = Sim::<Later>::new(&[]).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert_eq!(r.current[0], 9.);
    assert_eq!(r.previous[0], 9.);
    assert_eq!(r.slots, [0]);
    f.reset();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.meshes.len(), 1);
}
