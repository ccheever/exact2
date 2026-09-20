use clod_bake::{Mesh, ao, bake, procedural};
use clod_format::Config;
#[test]
fn crease_is_darker_than_convex_bump_and_bytes_repeat() {
    let mut failures = Vec::new();
    let mut mesh = Mesh::default();
    // A tessellated inside corner: a floor at z=0 and wall at x=0.
    for wall in [false, true] {
        let base = mesh.positions.len() as u32;
        for j in 0..=40 {
            for i in 0..=40 {
                let u = i as f32 / 40.0;
                let v = j as f32 / 40.0;
                mesh.positions
                    .push(if wall { [0.0, v, u] } else { [u, v, 0.0] });
                mesh.normals.push(if wall {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 0.0, 1.0]
                });
            }
        }
        for j in 0..40 {
            for i in 0..40 {
                let a = base + j * 41 + i;
                mesh.indices
                    .extend_from_slice(&[a, a + 1, a + 41, a + 1, a + 42, a + 41]);
            }
        }
    }
    let (crease, proxy) = ao::bake(&mesh).unwrap();
    let mut bump = procedural::octasphere(3).unwrap();
    // Convex reference with radial normals, removing the procedural relief.
    for p in &mut bump.positions {
        *p = clod_bake::normalize(*p);
    }
    bump.normals = bump.positions.clone();
    let (convex, _) = ao::bake(&bump).unwrap();
    let crease_value = crease[20 * 41 + 1];
    let convex_mean = convex.iter().map(|&v| v as u64).sum::<u64>() as f64 / convex.len() as f64;
    if crease_value as f64 >= convex_mean {
        failures.push("inside corner must be darker than convex bump");
    }
    let colors: Vec<_> = crease.iter().map(|&a| [255, 255, 255, a]).collect();
    mesh.colors = Some(colors);
    let a = bake(&mut mesh, Config::default(), [1; 32]).unwrap();
    let (repeat, _) = ao::bake(&mesh).unwrap();
    if crease != repeat {
        failures.push("AO samples changed on repeat");
    }
    let b = bake(&mut mesh, Config::default(), [1; 32]).unwrap();
    if a.bytes != b.bytes {
        failures.push("AO bake bytes changed on repeat");
    }
    println!(
        "ao vertices={} proxy_triangles={} crease={} convex_mean={} repeat_bytes={} failures={failures:?}",
        mesh.positions.len(),
        proxy,
        crease_value,
        convex_mean,
        a.bytes.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
}
