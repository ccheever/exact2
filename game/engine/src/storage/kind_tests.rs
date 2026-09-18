//! Allocation instrumentation stays in storage, the engine's unsafe boundary.
use crate::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Instant;

struct Allocator;
thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get().map(|n| n + 1)));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get().map(|n| n + 1)));
        unsafe { System.realloc(ptr, layout, size) }
    }
}
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|n| n.set(Some(0)));
    f();
    ALLOCATIONS.with(|n| n.replace(None).unwrap())
}

#[derive(Kind)]
struct Bulb {
    light: PointLight,
}
#[derive(Default, Component)]
struct Wick {
    bulb: Id<Bulb>,
}
#[derive(Kind)]
struct Lamp {
    #[child("bulb", bulb)]
    wick: Wick,
}
#[test]
fn idle_kind_paths_allocate_nothing() {
    let mut w = World::new(60, 0);
    // Mostly nonmembers, including the owning component without the full join.
    for _ in 0..2000 {
        w.spawn(Transform::default());
    }
    let bulb = w.spawn_kind(
        "lamp/bulb",
        Bulb {
            light: PointLight::default(),
        },
    );
    let lamp = w.spawn_kind(
        "lamp",
        Lamp {
            wick: Wick::default(),
        },
    );
    let tick = || {
        assert_eq!(w.row(lamp).unwrap().wick.bulb, bulb);
        w.edit(bulb, |r| {
            std::hint::black_box(r.light.intensity);
        });
        let mut seen = 0;
        for r in w.rows::<Lamp>() {
            assert_eq!(r.wick.bulb, bulb);
            seen += 1;
        }
        for r in w.rows_mut::<Lamp>() {
            assert_eq!(r.wick.bulb, bulb);
            seen += 1;
        }
        assert_eq!(seen, 2);
        assert_eq!(w.the::<Lamp>(), lamp);
    };
    for _ in 0..100 {
        assert_eq!(allocations(tick), 0, "idle kind tick allocated");
    }
    // Negative control for the counter itself and for empty iteration above.
    assert!(
        allocations(|| {
            std::hint::black_box(Box::new(17));
        }) > 0
    );
}

#[derive(Kind)]
struct Sparse {
    #[read]
    transform: Transform,
    light: PointLight,
    material: Option<Material>,
}
fn sparse_world() -> World {
    let mut w = World::new(60, 0);
    for i in 0..200_000 {
        let mut e = w.spawn(Transform::at(i as f32, 0.0, 0.0));
        if i % 7 == 0 {
            w.despawn(e);
            e = w.spawn(Transform::at(i as f32, 0.0, 0.0));
        }
        if i % 100 == 37 {
            w.insert(e, PointLight::default());
            if i % 200 == 37 {
                w.insert(e, Material::default());
            }
        }
    }
    w
}
fn typed(w: &World) -> (usize, u64) {
    let mut result = (0, 0);
    for mut row in w.rows_mut::<Sparse>() {
        row.light.intensity += 1.0;
        if let Some(mut material) = row.material {
            material.roughness = 0.5;
        }
        result.0 += 1;
        result.1 += row.transform.position.x as u64;
    }
    result
}
fn raw(w: &World) -> (usize, u64) {
    let mut result = (0, 0);
    for (transform, mut light, material) in
        w.query::<(&Transform, &mut PointLight, Option<&mut Material>)>()
    {
        light.intensity += 1.0;
        if let Some(mut material) = material {
            material.roughness = 0.5;
        }
        result.0 += 1;
        result.1 += transform.position.x as u64;
    }
    result
}
#[test]
fn sparse_200k_one_percent_matches_raw_query_without_allocations() {
    let w = sparse_world();
    let expected = (2000, (0..200_000u64).filter(|i| i % 100 == 37).sum());
    assert_eq!(allocations(|| assert_eq!(typed(&w), expected)), 0);
    assert_eq!(raw(&w), expected);
    let mut previous = None;
    for row in w.rows::<Sparse>() {
        let index = row.id.entity().index();
        assert!(previous.is_none_or(|old| old < index));
        previous = Some(index);
        assert_eq!(row.light.intensity, PointLight::default().intensity + 2.0);
    }
}
#[test]
#[ignore = "manual sparse 200k timing, median of five alternating samples"]
fn sparse_200k_one_percent_timing() {
    let w = sparse_world();
    let mut samples = [Vec::new(), Vec::new()];
    for sample in 0..5 {
        for mode in [sample % 2, 1 - sample % 2] {
            let start = Instant::now();
            for _ in 0..1000 {
                let value = if mode == 0 { raw(&w) } else { typed(&w) };
                assert_eq!(std::hint::black_box(value).0, 2000);
            }
            samples[mode].push(start.elapsed().as_nanos() / 1000);
        }
    }
    for sample in &mut samples {
        sample.sort_unstable();
    }
    let raw = samples[0][2];
    let typed = samples[1][2];
    println!(
        "200k slots / 1% members ns/pass: raw {:?}, typed {:?}; ratio {:.4}",
        samples[0],
        samples[1],
        typed as f64 / raw as f64
    );
    assert!(typed <= raw * 110 / 100, "kind exceeds raw query by 10%");
}
