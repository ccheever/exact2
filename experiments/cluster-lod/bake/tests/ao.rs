use clod_bake::{Mesh, ao, bake, procedural};
use clod_format::Config;
#[test]
fn crease_is_darker_than_convex_bump_and_bytes_repeat() {
    let mut failures = Vec::new();
    let mut mesh = corner(40);
    let mut small = mesh.clone();
    for p in &mut small.positions {
        *p = p.map(|x| x * 1e-9);
    }
    let (small_ao, _) = ao::bake(&small).unwrap();
    let (crease, proxy) = ao::bake(&mesh).unwrap();
    let mut bump = procedural::octasphere(3).unwrap();
    // Convex reference with radial normals, removing the procedural relief.
    for p in &mut bump.positions {
        *p = clod_bake::normalize(*p);
    }
    bump.normals = bump.positions.clone();
    let (convex, _) = ao::bake(&bump).unwrap();
    let scaled_differences = small_ao.iter().zip(&crease).filter(|(a, b)| a != b).count();
    println!("ao_scaled_differing_vertices={scaled_differences}");
    if scaled_differences > 0 {
        failures.push("AO bytes changed under 1e-9 unit scaling");
    }
    let small_value = small_ao[20 * 41 + 1];
    println!("ao_scaled_crease={small_value}");
    if small_value >= 250 {
        failures.push("1e-9 crease lost occlusion");
    }
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

fn corner(resolution: u32) -> Mesh {
    let mut mesh = Mesh::default();
    // A tessellated inside corner: a floor at z=0 and wall at x=0.
    for wall in [false, true] {
        let base = mesh.positions.len() as u32;
        for j in 0..=resolution {
            for i in 0..=resolution {
                let u = i as f32 / resolution as f32;
                let v = j as f32 / resolution as f32;
                mesh.positions
                    .push(if wall { [0.0, v, u] } else { [u, v, 0.0] });
                mesh.normals.push(if wall {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 0.0, 1.0]
                });
            }
        }
        for j in 0..resolution {
            for i in 0..resolution {
                let a = base + j * (resolution + 1) + i;
                let triangles = if wall {
                    [
                        a,
                        a + (resolution + 1),
                        a + 1,
                        a + 1,
                        a + (resolution + 1),
                        a + (resolution + 2),
                    ]
                } else {
                    [
                        a,
                        a + 1,
                        a + (resolution + 1),
                        a + 1,
                        a + (resolution + 2),
                        a + (resolution + 1),
                    ]
                };
                mesh.indices.extend_from_slice(&triangles);
            }
        }
    }
    mesh
}

#[test]
fn proxy_crease_is_deterministic_across_workers() {
    let mut mesh = corner(256);
    let mut failures = vec![];
    let mut previous = None;
    for workers in [1, 1, 4] {
        let (values, proxy) = ao::bake_with_workers(&mesh, workers).unwrap();
        let crease = values[128 * 257 + 1];
        if proxy >= mesh.indices.len() as u32 / 3 || crease >= 250 {
            failures.push(format!("proxy={proxy} crease={crease}"));
        }
        mesh.colors = Some(values.iter().map(|&a| [255, 255, 255, a]).collect());
        let bytes = bake(&mut mesh, Config::default(), [2; 32]).unwrap().bytes;
        if let Some((prior_values, prior_bytes)) = &previous
            && (*prior_values != values || *prior_bytes != bytes)
        {
            failures.push(format!("worker {workers} changed bytes"));
        }
        println!(
            "ao_proxy workers={workers} source_triangles={} proxy_triangles={proxy} crease={crease} baked_bytes={}",
            mesh.indices.len() / 3,
            bytes.len()
        );
        previous = Some((values, bytes));
    }
    let mut bump = procedural::octasphere(8).unwrap();
    for p in &mut bump.positions {
        *p = clod_bake::normalize(*p);
    }
    bump.normals = bump.positions.clone();
    let (values, proxy) = ao::bake_with_workers(&bump, 4).unwrap();
    let mean = values.iter().map(|&v| v as f64).sum::<f64>() / values.len() as f64;
    println!(
        "ao_proxy convex_source={} convex_proxy={proxy} convex_mean={mean} repeat_runs=3 failures={failures:?}",
        bump.indices.len() / 3
    );
    if mean < 250.0 {
        failures.push(format!("convex_mean={mean}"));
    }
    assert!(failures.is_empty(), "{failures:?}");
}
