use super::*;
use exact_game::{
    Camera, Clock, DirectionalLight, Entity, Game, Input, PointLight, Quat, Sim, Vec3,
};

#[derive(Debug, PartialEq)]
enum Call {
    Begin,
    Transform(u32, usize, bool),
    Material(u32, usize),
    Previous(u32, usize),
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
    fn begin_tick(&mut self) {
        self.call(Call::Begin);
        std::mem::swap(&mut self.current, &mut self.previous);
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
    fn previous(&mut self, first: u32, values: &[f32]) -> Result<(), RenderError> {
        self.call(Call::Previous(first, values.len()));
        let start = first as usize * 10;
        let end = start + values.len();
        if self.previous.len() < end {
            self.previous.resize(end, 0.);
        }
        self.previous[start..end].copy_from_slice(values);
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
    type Args = ();
    const ID: &'static str = "feed-test";
    fn setup(w: &mut World, _: &Self::Args) {
        w.spawn((Transform::default(), Mesh::cube(1.0), Material::default()));
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        for (_, t) in w.query::<&mut Transform>().iter() {
            t.position.x += 1.0;
        }
    }
}
struct Stop;
impl Game for Stop {
    type Args = ();
    const ID: &'static str = "feed-stop";
    fn setup(w: &mut World, a: &Self::Args) {
        Moving::setup(w, a)
    }
    fn tick(w: &mut World, i: &Input, args: &Self::Args) {
        if w.tick() == 0 {
            Moving::tick(w, i, args);
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
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    r.calls.clear();
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert!(r.calls.contains(&Call::Begin));
    assert_ne!(r.previous, r.current); // Includes the setup-to-first-tick stamp collision.
    assert!(!r.calls.contains(&Call::Batches));
    r.calls.clear();
    sim.advance_with(34., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert_eq!(r.calls, [Call::Begin, Call::Transform(0, PAGE * 10, false)]);
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
    let b = w.spawn((Transform::at(0., 2., 0.), Parent(a), Mesh::cube(1.0)));
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
    assert!(r.calls.contains(&Call::Previous(b.index(), 10)));
    assert_eq!(r.position(b, true), r.position(b, false));
    let e = w.spawn((Transform::at(50., 0., 0.), Mesh::cube(1.0)));
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.position(e, true), Vec3::new(50., 0., 0.));
}
#[test]
fn structure_and_visibility_rebuild_but_movement_does_not() {
    let mut w = World::new(60, 0);
    let a = w.spawn((Transform::default(), Mesh::cube(1.0)));
    let b = w.spawn((Transform::default(), Mesh::sphere(1.0), Visible(false)));
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
    let c = w.spawn((Transform::at(7., 0., 0.), Mesh::cube(1.0)));
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
        Material::grid([0.2, 0.3, 0.4], 2.0).emissive(2., 3., 4.),
    ));
    let b = w.spawn((Transform::default(), Mesh::cube(1.0)));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Material(0, PAGE * 12)));
    assert_eq!(
        &r.materials[..9],
        &[0.2, 0.3, 0.4, -2., 0., 0.5, 2., 3., 4.]
    );
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
    let mut sim = Sim::<Moving>::new(()).unwrap();
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
    assert_eq!(
        e,
        RenderError::Capacity {
            arena: "transforms",
            slot: PAGE as u64,
            limit: PAGE as u64
        }
    );
    assert!(r.calls.is_empty());
    r.limit += 1;
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Transform(0, (PAGE + 1) * 10, true)));
}
#[test]
fn primitive_dimensions_are_instance_data_and_capsules_deform_exactly() {
    let mut w = World::new(60, 0);
    for mesh in [
        Mesh::sphere(1.0),
        Mesh::cylinder(1.0, 1.0),
        Mesh::plane(40., 40.),
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
        Mesh::cube(1.0),
    ] {
        w.spawn((Transform::default(), mesh));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.meshes.len(), 5);
    for (i, expected) in [
        (0, Vec3::ONE),
        (1, Vec3::new(1., 0.5, 1.)),
        (2, Vec3::new(20., 0., 20.)),
        (3, Vec3::new(0.4, 0.5, 0.4)),
    ] {
        let extent = r.meshes[i]
            .0
            .iter()
            .map(|v| {
                let dims = Vec3::from_slice(&r.materials[i * 12 + 9..i * 12 + 12]);
                let scale = if v.uv[1] == 1.0 {
                    Vec3::splat(dims.x)
                } else {
                    dims
                };
                (Vec3::from_array(v.position) * scale + Vec3::Y * v.uv[0] * dims.y).abs()
            })
            .fold(Vec3::ZERO, Vec3::max);
        assert!(
            extent.distance(expected) < 1e-5,
            "{extent:?} != {expected:?}"
        );
    }
}
#[test]
fn camera_slerps_and_nearest_lights_interpolate_without_frame_scans() {
    let mut sim = Sim::<Moving>::new(()).unwrap();
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
    assert_eq!(input.lights.len(), 20);
    assert_eq!(input.lights[19].position.x, 20.25);
    assert_eq!(input.lights[0].position.x, 1.25);
    assert_eq!(input.lights[15].position.x, 16.25);
    assert!(input.sun.unwrap().shadows.is_some());
    assert!(input.environment.bloom.is_some());
    assert_eq!(
        input.environment.fog,
        exact_game::Environment::default().fog
    );
    sim.world().get_mut::<Transform>(camera).unwrap().rotation = Quat::from_rotation_y(1.0);
    f.feed_to(sim.world(), &mut r).unwrap();
    let input = f.frame(sim.world(), 0.5, 2.);
    let forward = input.view.inverse().transform_vector3(-Vec3::Z);
    assert!(forward.distance(Quat::from_rotation_y(0.5) * -Vec3::Z) < 1e-5);
    sim.world_mut()
        .teleport(lights[19], Transform::at(0.9, 0., 0.));
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(
        f.frame(sim.world(), 0.5, 2.).lights[0].position,
        Vec3::new(0.9, 0., 0.)
    );
}

#[allow(unsafe_code)]
pub(crate) mod allocations {
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
    let mut sim = Sim::<Moving>::new(()).unwrap();
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
    sim.advance(0., Clock::Live);
    sim.advance_with(17., Clock::Live, |w, _| f.feed_to(w, &mut r).unwrap());
    let mut trace = crate::trace::Trace::new(sim.world(), "#0", 256).unwrap();
    let count = allocations::count(|| {
        for i in 2..240 {
            sim.advance_with(i as f64 * 1000. / 60. + 0.001, Clock::Live, |w, left| {
                if left < 2 {
                    f.feed_to(w, &mut r).unwrap();
                    trace.feed(w);
                }
            });
            let frame = f.frame(sim.world(), 0.5, 16. / 9.);
            std::hint::black_box(frame.camera_position);
            trace.frame(i as f64, 0.5, 1, f.trace_camera(0.5), [0.; 3]);
        }
    });
    assert_eq!(count, 0);
    assert!(sim.world().contains(camera));
}

#[test]
fn first_transform_on_an_older_entity_initializes_history_and_reset_reuses_meshes() {
    struct Later;
    impl Game for Later {
        type Args = ();
        const ID: &'static str = "late-pose";
        fn setup(w: &mut World, _: &Self::Args) {
            w.spawn(Mesh::cube(1.0));
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            let e = w.query::<&Mesh>().iter().next().unwrap().0;
            if !w.has::<Transform>(e) {
                w.insert(e, Transform::at(9., 0., 0.));
            }
        }
    }
    let mut sim = Sim::<Later>::new(()).unwrap();
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

#[test]
fn ancestor_teleports_and_parent_edits_snap_mesh_camera_and_lights() {
    struct Empty;
    impl Game for Empty {
        type Args = ();
        const ID: &'static str = "parent-snap";
        fn setup(_: &mut World, _: &Self::Args) {}
        fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
    }
    let mut sim = Sim::<Empty>::new(()).unwrap();
    let w = sim.world_mut();
    let root = w.spawn(Transform::default());
    let middle = w.spawn((Transform::at(1., 0., 0.), Parent(root)));
    let mesh = w.spawn((Transform::at(1., 0., 0.), Parent(middle), Mesh::cube(1.0)));
    let camera = w.spawn((Transform::at(0., 0., 8.), Parent(middle), Camera::default()));
    let light = w.spawn((Transform::default(), Parent(middle), PointLight::default()));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    sim.world_mut().teleport(root, Transform::at(20., 0., 0.));
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(mesh, true).x, 22.);
    assert_eq!(r.position(mesh, false).x, 22.);
    let frame = f.frame(sim.world(), 0.5, 1.);
    assert_eq!(frame.camera_position.x, 21.);
    assert_eq!(frame.lights[0].position.x, 21.);
    // Clear freshness, then remove/reinsert Parent with no fresh entity involved.
    sim.advance_with(34., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    sim.world_mut().remove::<Parent>(middle);
    sim.world_mut().propagate();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(middle, true).x, 1.);
    assert_eq!(r.position(mesh, true).x, 2.);
    assert_eq!(f.frame(sim.world(), 0.5, 1.).camera_position.x, 1.);
    sim.world_mut().insert(middle, Parent(root));
    sim.world_mut().propagate();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(mesh, true).x, 22.);
    assert_eq!(r.position(camera, true).x, 21.);
    assert_eq!(r.position(light, true).x, 21.);
}

#[test]
fn load_invalidates_equal_revisions_and_does_not_change_save_or_hash() {
    let mut w = World::new(60, 0);
    let e = w.spawn((
        Transform::at(1., 0., 0.),
        Mesh::cube(1.0),
        Camera::default(),
    ));
    let a = w.save();
    w.get_mut::<Transform>(e).unwrap().position.x = 8.;
    let b = w.save();
    w.load(&a).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let revision = w.revision::<Transform>();
    let generation = w.presentation_generation();
    w.load(&b).unwrap();
    assert_eq!(w.revision::<Transform>(), revision);
    assert!(w.presentation_generation() > generation);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.position(e, true).x, 8.);
    assert_eq!(r.position(e, false).x, 8.);
    assert_eq!(f.frame(&w, 0.5, 1.).camera_position.x, 8.);
    let hash = w.hash();
    w.load(&b).unwrap();
    assert_eq!(w.hash(), hash);
    assert_eq!(w.save(), b);
}

#[test]
fn sun_skips_transformless_light_and_slerps_each_tick() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    sim.world_mut().spawn(DirectionalLight::default());
    let sun = sim
        .world_mut()
        .spawn((Transform::default(), DirectionalLight::default()));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert!(f.frame(sim.world(), 0., 1.).sun.is_some());
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    sim.world().get_mut::<Transform>(sun).unwrap().rotation = Quat::from_rotation_y(1.);
    f.feed_to(sim.world(), &mut r).unwrap();
    let direction = f.frame(sim.world(), 0.5, 1.).sun.unwrap().direction;
    assert!(direction.distance(Quat::from_rotation_y(0.5) * -Vec3::Z) < 1e-5);
}

#[test]
fn zero_quaternion_cpu_pose_is_finite_identity() {
    let mut w = World::new(60, 0);
    let e = w.spawn(Transform {
        rotation: Quat::from_xyzw(0., 0., 0., 0.),
        ..Default::default()
    });
    assert_eq!(scene::pose(&w, e).unwrap().rotation, Quat::IDENTITY);
    // Degenerate decomposition must not normalize zero/non-finite into NaN either.
    w.get_mut::<Transform>(e).unwrap().scale = Vec3::ZERO;
    assert_eq!(scene::pose(&w, e).unwrap().rotation, Quat::IDENTITY);
}

#[test]
fn light_selection_matches_restored_current_state_with_twenty_lights() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let w = sim.world_mut();
    let camera = w.spawn((Transform::default(), Camera::default()));
    for i in 0..20 {
        w.spawn((
            Transform::at(if i < 16 { -10. } else { 10.1 }, i as f32 * 0.001, 0.),
            PointLight::default(),
        ));
    }
    let mut running = Feed::default();
    let mut r = Recording::default();
    running.feed_to(w, &mut r).unwrap();
    sim.world().get_mut::<Transform>(camera).unwrap().position.x = 0.2;
    running.feed_to(sim.world(), &mut r).unwrap();
    let save = sim.save().unwrap();
    sim.restore(&save).unwrap();
    let mut restored = Feed::default();
    restored.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(
        running.scene.lights_for_test(),
        restored.scene.lights_for_test()
    );
}

#[test]
fn fixture_writes_one_or_two_pages_while_moving_and_none_at_rest() {
    let mut sim = Sim::<crate::test_game::Fixture>::from_values(&[
        exact_game::Value::Number(7.),
        exact_game::Value::Bool(false),
        exact_game::Value::Bool(false),
    ])
    .unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.input(exact_game::InputEvent::Key {
        code: "KeyW".into(),
        down: true,
        at_ms: 0.,
    });
    let writes = |r: &Recording| {
        r.calls
            .iter()
            .filter(|c| matches!(c, Call::Transform(..) | Call::Material(..)))
            .count()
    };
    for tick in 1..=96 {
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        assert_eq!(writes(&r), 1);
        assert!(r.calls.contains(&Call::Transform(0, PAGE * 10, false)));
    }
    sim.input(exact_game::InputEvent::Key {
        code: "KeyW".into(),
        down: false,
        at_ms: 1600.001,
    });
    sim.input(exact_game::InputEvent::Key {
        code: "KeyE".into(),
        down: true,
        at_ms: 1600.001,
    });
    let mut glowing = 0;
    let mut previous_emissive = 0.;
    for tick in 97..=360 {
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        assert!(writes(&r) <= 1);
        assert!(!r.calls.iter().any(|c| matches!(c, Call::Material(..))));
        let frame = f.frame(sim.world(), 1., 1.);
        let emissive = frame.glows[0].material_at(frame.seconds)[6];
        glowing += usize::from(emissive > previous_emissive);
        previous_emissive = emissive;
        if tick > 300 {
            assert_eq!(writes(&r), 0);
        }
    }
    assert!(glowing > 1, "the fixture sphere must have glowed");
    eprintln!("fixture writes: moving=1 transform page/tick; glow sampled without material-page writes; settled=0");
}

#[test]
fn identical_parent_writes_stop_uploading_settled_transform_pages() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let w = sim.world_mut();
    let root = w.spawn(Transform::default());
    let mut entities = Vec::new();
    for i in 2..PAGE * 40 {
        entities.push(w.spawn((Transform::at(i as f32, 0., 0.), Material::default())));
    }
    let child = entities[PAGE];
    w.insert(child, Parent(root));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    assert_eq!(
        r.calls
            .iter()
            .filter(|c| matches!(c, Call::Transform(..)))
            .count(),
        1
    );
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    // Repeated writes of identical values still bump the column revision.
    for tick in 2..100 {
        sim.world().get_mut::<Transform>(root).unwrap().position.x = 1.;
        sim.world_mut().propagate();
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        if tick > 40 {
            assert!(!r.calls.iter().any(|c| matches!(c, Call::Transform(..))));
        }
    }
    assert_eq!(r.position(child, true), r.position(child, false));
}

#[test]
fn several_pages_of_still_transforms_with_moving_camera_write_only_its_page() {
    struct CameraOnly;
    impl Game for CameraOnly {
        type Args = ();
        const ID: &'static str = "large-still-writes";
        fn setup(w: &mut World, _: &Self::Args) {
            for _ in 0..(PAGE * 3 + 17) {
                w.spawn(Transform::default());
            }
            w.spawn((Transform::default(), Camera::default()));
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            for (_, (_, t)) in w.query::<(&Camera, &mut Transform)>().iter() {
                t.position.x += 1.;
            }
        }
    }
    let mut sim = Sim::<CameraOnly>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    for tick in 1..=40 {
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        if tick >= 35 {
            assert_eq!(
                r.calls,
                [
                    Call::Begin,
                    Call::Transform((PAGE * 3) as u32, PAGE * 10, false)
                ]
            );
        }
    }
    eprintln!(
        "3 full pages plus a partial page of still entities + moving camera: 1 transform page/write per tick, 0 material writes"
    );
}

#[test]
fn animated_dimensions_never_grow_geometry_and_all_spheres_batch_together() {
    let mut w = World::new(60, 0);
    let e = w.spawn_named("pulse", (Transform::default(), Mesh::sphere(0.5)));
    for i in 1..7 {
        w.spawn((Transform::default(), Mesh::sphere(i as f32)));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.meshes.len(), 1);
    assert_eq!(f.batches.len(), 1);
    assert_eq!(f.slots.len(), 7);
    for radius in [0.01, 0.5, 4.599, 2.0, 0.5] {
        *w.get_mut::<Mesh>(e).unwrap() = Mesh::sphere(radius);
        f.feed_to(&w, &mut r).unwrap();
        assert_eq!(r.meshes.len(), 1);
    }
    assert_eq!(f.batches.len(), 1);
    assert_eq!(r.materials[9], 1.0);
}
#[test]
fn asset_mesh_waits_without_inventing_geometry() {
    let mut w = World::new(60, 0);
    w.spawn((Transform::default(), Mesh::asset("castle")));
    let mut feed = Feed::default();
    let mut r = Recording::default();
    feed.feed_to(&w, &mut r).unwrap();
    assert!(feed.batches.is_empty());
}

#[test]
fn revealing_a_hidden_primitive_keeps_its_dimensions() {
    let mut w = World::new(60, 0);
    let e = w.spawn((Transform::default(), Mesh::sphere(2.0), Visible(false)));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(&r.materials[9..12], &[4.0; 3]);
    assert!(f.batches.is_empty());
    w.get_mut::<Visible>(e).unwrap().0 = true;
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(f.batches.len(), 1);
    assert_eq!(&r.materials[9..12], &[4.0; 3]);
}

#[test]
#[ignore = "release CPU feed diagnostic; recording backend, no GPU"]
fn feed_cpu_cost() {
    use std::time::Instant;
    for n in [200_000, 500_000] {
        for moving in [n, n / 100, 0] {
            let mut w = World::new(60, 0);
            for _ in 0..n {
                w.spawn((Transform::default(), Mesh::cube(1.0), Material::default()));
            }
            let camera = w.spawn((Transform::default(), Camera::default()));
            w.load(&w.save()).unwrap(); // Clear setup-only fresh entities before steady measurements.
            let mut f = Feed::default();
            let mut r = Recording {
                record: false,
                ..Recording::default()
            };
            f.feed_to(&w, &mut r).unwrap();
            let mut samples = Vec::new();
            for tick in 0..300 {
                for (_, t) in w.query::<&mut Transform>().iter().take(moving) {
                    t.position.x += 1.;
                }
                w.get_mut::<Transform>(camera).unwrap().position.x += 1.;
                let start = Instant::now();
                f.feed_to(&w, &mut r).unwrap();
                if tick >= 60 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "CPU_FEED n={n} moving={moving} p50={:.4} p95={:.4}",
                samples[119], samples[227]
            );
        }
    }
}

#[test]
fn renderer_defaults_match_world_and_negative_alpha_does_not_enable_grid() {
    assert_eq!(
        crate::Environment::default().fog,
        exact_game::Environment::default().fog
    );
    let mut material = Material::default();
    material.color[3] = -2.0;
    assert!(material_floats(material)[3] >= 0.0);
    material.grid_spacing = 2.0;
    assert_eq!(material_floats(material)[3], -2.0);
}

#[test]
fn aspect_only_feed_preserves_authored_integer_camera_height() {
    let mut w = World::new(60, 0);
    w.spawn((
        Transform::default(),
        Camera::orthographic(180.).integer_scale(),
    ));
    w.propagate();
    let mut f = Feed::default();
    f.feed_to(&w, &mut Recording::default()).unwrap();
    assert!((f.frame(&w, 1., 2.).proj.y_axis.y - 2. / 180.).abs() < 1e-7);
    assert!((f.frame_pixels(&w, 1., (800., 400.)).proj.y_axis.y - 2. / 200.).abs() < 1e-7);
}

#[test]
fn e10_glow_samples_frame_time_without_writing_saved_materials() {
    use exact_game::{Glow, Now, Tween};
    struct Empty;
    impl Game for Empty {
        const ID: &'static str = "glow-frame";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Empty>::new(()).unwrap();
    sim.run(250.);
    let w = sim.world_mut();
    let e = w.spawn((
        Transform::default(),
        Mesh::sphere(0.5),
        Material::glow([3., 2., 1.]),
        Glow(Tween::new(0.)),
    ));
    w.get_mut::<Glow>(e)
        .unwrap()
        .0
        .to(Now { tick: 0, hz: 60 }, 1., 0.5);
    let mut feed = Feed::default();
    let mut r = Recording::default();
    feed.feed_to(w, &mut r).unwrap();
    let frame = feed.frame(w, 1., 1.);
    assert_eq!(frame.glows.len(), 1);
    let glow = &frame.glows[0];
    assert_eq!(&glow.material_at(0.)[6..9], &[0., 0., 0.]);
    assert_eq!(&glow.material_at(0.25)[6..9], &[1.5, 1., 0.5]);
    assert_eq!(&glow.material_at(0.5)[6..9], &[3., 2., 1.]);
    let saved = w.hash();
    let frame = feed.frame(w, 0.5, 1.);
    let expected = w.get::<Glow>(e).unwrap().0.value_at(frame.seconds, w.hz());
    assert!((frame.seconds - 14.5 / 60.).abs() < 1e-9);
    assert_eq!(frame.glows[0].material_at(frame.seconds)[8], expected);
    assert!(
        expected
            < w.get::<Glow>(e).unwrap().0.value(Now {
                tick: w.tick(),
                hz: w.hz()
            })
    );
    assert_eq!(w.hash(), saved);
    assert_eq!(w.get::<Material>(e).unwrap().emissive, [3., 2., 1.]);
    w.remove::<Glow>(e);
    feed.feed_to(w, &mut r).unwrap();
    assert!(feed.frame(w, 1., 1.).glows.is_empty());
    assert_eq!(&r.materials[6..9], &[3., 2., 1.]);
}

#[test]
fn e11_glow_identity_preserves_authored_emission_and_shoulders_only_added_light() {
    let glow = crate::GlowInput {
        slot: 0,
        material: [0., 0., 0., 0., 0., 0., 3., 2., 1., 0., 0., 0.],
        tween: exact_game::Tween::new(1.),
        hz: 60,
        model: false,
    };
    assert_eq!(&glow.material_at(0.)[6..9], &[3., 2., 1.]);
    let hot = crate::GlowInput {
        tween: exact_game::Tween::new(2.),
        ..glow
    };
    assert_eq!(&hot.material_at(0.)[6..9], &[3. + 3. / 2.5, 3., 2.]);
}

#[test]
fn e11_settle_immediately_presents_the_completed_glow() {
    struct Fade;
    impl Game for Fade {
        const ID: &'static str = "settle-glow";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let mut tween = exact_game::Tween::new(0.);
            tween.to(w.now(), 1., 0.5);
            w.spawn((
                Transform::default(),
                Mesh::sphere(0.5),
                Material::glow([1.; 3]),
                exact_game::Glow(tween),
            ));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Fade>::new(()).unwrap();
    assert!(sim.settle());
    let mut feed = Feed::default();
    let mut recording = Recording::default();
    feed.feed_to(sim.world(), &mut recording).unwrap();
    let frame = feed.frame(sim.world(), sim.alpha(), 1.);
    assert_eq!(&frame.glows[0].material_at(frame.seconds)[6..9], &[1.; 3]);
}

#[test]
fn model_glow_scales_baked_emission_without_a_material_component() {
    let mut w = World::new(60, 0);
    let e = w.spawn((
        Transform::default(),
        Mesh::asset("lamp.model"),
        exact_game::Glow(exact_game::Tween::new(0.25)),
    ));
    let hash = w.hash();
    let mut feed = Feed::default();
    feed.feed_to(&w, &mut Recording::default()).unwrap();
    let frame = feed.frame(&w, 1., 1.);
    assert_eq!(frame.glows.len(), 1);
    assert_eq!(frame.glows[0].material_at(0.)[9], 0.25);
    assert_eq!(w.hash(), hash);
    w.remove::<exact_game::Glow>(e);
    let mut restored = Recording::default();
    feed.feed_to(&w, &mut restored).unwrap();
    assert!(restored
        .calls
        .iter()
        .any(|c| matches!(c, Call::Material(..))));
    assert!(feed.frame(&w, 1., 1.).glows.is_empty());
}

#[test]
fn a_slot_reoccupied_between_feeds_takes_its_current_pose_as_history() {
    struct Recycle;
    impl Game for Recycle {
        type Args = ();
        const ID: &'static str = "feed-recycle";
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("old", (Transform::at(5., 0., 0.), Mesh::cube(1.0)));
            w.spawn((Transform::default(), Mesh::cube(1.0)));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            match w.tick() {
                3 => {
                    let old = w.named("old").unwrap();
                    w.despawn(old);
                }
                4 => {
                    w.spawn_named("new", (Transform::at(-7., 0., 0.), Mesh::cube(1.0)));
                }
                _ => {}
            }
        }
    }
    let mut sim = Sim::<Recycle>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    for _ in 0..3 {
        sim.run(1000. / 60.);
        f.feed_to(sim.world(), &mut r).unwrap();
    }
    // Ticks 3-5 run without a feed: the old occupant leaves, the new one
    // arrives and stops being fresh before the presentation sees it.
    sim.run(3000. / 60.);
    let new = sim.world().named("new").unwrap();
    assert_eq!(new.index(), 0, "the slot is recycled");
    assert!(!sim.world().is_fresh(new));
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(new, false).x, -7.);
    assert_eq!(
        r.position(new, true).x,
        -7.,
        "no frame interpolates from the old occupant"
    );
}

#[test]
fn mouse_look_turns_the_drawn_camera_and_its_children_about_the_eye() {
    let mut w = World::new(60, 0);
    let look = exact_game::MouseLook {
        yaw_per_point: -0.01,
        pitch_per_point: -0.01,
        pitch_limit: 0.5,
    };
    let camera = w.spawn((Transform::at(1., 2., 3.), Camera::default(), look));
    let gun = w.spawn((
        Transform::at(0.3, -0.2, -0.5),
        Parent(camera),
        Mesh::cube(0.1),
    ));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert!(
        f.frame(&w, 1., 1.).attachments.is_empty(),
        "no motion, no turn"
    );
    // 50 points left: half a radian of yaw, drawn only.
    f.unshown_motion(exact_game::Vec2::new(-50., 0.));
    let frame = f.frame(&w, 1., 1.);
    let eye = frame.view.inverse();
    assert!(frame.camera_position.distance(Vec3::new(1., 2., 3.)) < 1e-5);
    let forward = Quat::from_rotation_y(0.5) * -Vec3::Z;
    assert!((eye.transform_vector3(-Vec3::Z) - forward).length() < 1e-5);
    // The viewmodel turns with it: it stays put in the camera's view.
    let held = frame.attachments.iter().find(|a| a.entity == gun).unwrap();
    let in_view = (frame.view * held.matrix).w_axis.truncate();
    assert!((in_view - Vec3::new(0.3, -0.2, -0.5)).length() < 1e-5);
    // Pitch stops at the game's limit, and each frame's motion is its own.
    f.unshown_motion(exact_game::Vec2::new(0., -1000.));
    let up = f
        .frame(&w, 1., 1.)
        .view
        .inverse()
        .transform_vector3(-Vec3::Z);
    assert!((up.y - exact_game::math::sin(0.5)).abs() < 1e-4);
    assert!(f.frame(&w, 1., 1.).attachments.is_empty());
}

#[test]
fn a_parented_entity_that_gains_a_transform_later_draws_at_its_global_pose() {
    let mut w = World::new(60, 0);
    let parent = w.spawn((Transform::at(10., 0., 0.), Mesh::cube(1.0)));
    let child = w.spawn((Mesh::cube(1.0), Parent(parent)));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    // A later tick gives the child its local pose; Parent is unchanged.
    w.insert(child, Transform::at(0., 2., 0.));
    w.propagate();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.position(child, false), Vec3::new(10., 2., 0.));
}

#[test]
fn a_socket_followers_child_in_another_block_draws_at_its_tick_end_global() {
    use exact_game::{
        asset::{Content, Model, Node},
        Pose, SocketFollow,
    };
    struct Rig;
    impl Game for Rig {
        const ID: &'static str = "socket-child-feed";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(w: &mut World, _: &Input, _: &()) {
            // The rig animates: its Pose moves the head joint along +X.
            for (_, pose) in w.query::<&mut Pose>().iter() {
                pose.previous.clone_from(&pose.local);
                pose.local[0] += 0.25;
            }
        }
    }
    let model = Model {
        nodes: vec![Node {
            name: "head".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut sim = Sim::<Rig>::new(()).unwrap();
    sim.deliver_asset("rig.model", Ok(Content::Model(model.clone())))
        .unwrap();
    let w = sim.world_mut();
    let mut pose = Pose::default();
    pose.previous = exact_game::animation::bind_pose(&model);
    pose.local = pose.previous.clone();
    let rig = w.spawn((Transform::default(), Mesh::asset("rig.model"), pose));
    let follower = w.spawn((Transform::default(), SocketFollow::new(rig, "head")));
    for _ in 0..exact_game::PAGE {
        w.spawn(Transform::default());
    }
    let child = w.spawn((Transform::at(0., 1., 0.), Mesh::cube(0.2), Parent(follower)));
    assert_ne!(
        child.index() as usize / PAGE,
        follower.index() as usize / PAGE
    );
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    let mut xs = Vec::new();
    for _ in 0..6 {
        sim.run(1000. / 60.);
        f.feed_to(sim.world(), &mut r).unwrap();
        let global = Vec3::from(sim.world().global(child).unwrap().translation);
        assert!(
            r.position(child, false).distance(global) < 1e-5,
            "drawn {} vs tick-end global {global}",
            r.position(child, false)
        );
        xs.push(global.x);
    }
    assert!(
        xs.windows(2).all(|p| p[1] > p[0]),
        "the child moves: {xs:?}"
    );
}

#[test]
fn rewriting_the_same_offsets_is_not_a_pose_change() {
    // Game::present rewrites every Offset row each tick; only content counts.
    let mut w = World::new(60, 1);
    let e = w.spawn(Transform::default());
    w.insert(e, exact_game::Offset(Transform::at(1., 0., 0.)));
    let before = offsets(&w);
    w.remove::<exact_game::Offset>(e);
    w.insert(e, exact_game::Offset(Transform::at(1., 0., 0.)));
    assert_eq!(offsets(&w), before);
    w.insert(e, exact_game::Offset(Transform::at(1.5, 0., 0.)));
    assert_ne!(offsets(&w), before);
}
