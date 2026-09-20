use clod_bake::{bake, loaders, procedural};
use clod_format::{Cluster, Config, Group, Header, Node, Page, Reader};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{mem::size_of, path::PathBuf};

fn patch<T: bytemuck::Pod>(bytes: &mut [u8], offset: usize, change: impl FnOnce(&mut T)) {
    change(bytemuck::from_bytes_mut(
        &mut bytes[offset..offset + size_of::<T>()],
    ));
}
fn rehash(bytes: &mut [u8], offset: usize) {
    let p = *bytemuck::from_bytes::<Page>(&bytes[offset..offset + size_of::<Page>()]);
    let digest =
        Sha256::digest(&bytes[p.offset as usize..(p.offset + p.byte_length) as usize]).into();
    patch::<Page>(bytes, offset, |p| p.sha256 = digest);
}
#[test]
fn malformed_files_are_errors_not_panics() {
    let mut mesh = procedural::octasphere(4).expect("fixture");
    let baked = bake(&mut mesh, Config::default(), [7; 32]).expect("fixture bake");
    let bytes = baked.bytes;
    let reader = Reader::new(&bytes).expect("round trip");
    let h = *reader.header;
    let p = reader.pages[0];
    let mut cases = Vec::new();
    for len in [
        0,
        1,
        size_of::<Header>() - 1,
        bytes.len() / 2,
        bytes.len() - 1,
    ] {
        cases.push((format!("truncate_{len}"), bytes[..len].to_vec()));
    }
    for section in 0..5 {
        let mut b = bytes.clone();
        patch::<Header>(&mut b, 0, |h| match section {
            0 => h.clusters_offset = u64::MAX,
            1 => h.groups_offset = u64::MAX,
            2 => h.pages_offset = u64::MAX,
            3 => h.nodes_offset = u64::MAX,
            _ => h.geometry_offset = u64::MAX,
        });
        cases.push((format!("section_{section}"), b));
    }
    let mut corrupt = |name: &str, f: &dyn Fn(&mut Vec<u8>)| {
        let mut b = bytes.clone();
        f(&mut b);
        cases.push((name.to_owned(), b));
    };
    corrupt("magic", &|b| b[0] ^= 1);
    corrupt("version", &|b| patch::<Header>(b, 0, |h| h.version = 99));
    corrupt("count_overflow", &|b| {
        patch::<Header>(b, 0, |h| h.cluster_count = u32::MAX)
    });
    corrupt("geometry_digest", &|b| b[p.offset as usize] ^= 1);
    corrupt("page_length", &|b| {
        patch::<Page>(b, h.pages_offset as usize, |p| p.byte_length = u64::MAX)
    });
    corrupt("page_vertex_count", &|b| {
        patch::<Page>(b, h.pages_offset as usize, |p| p.vertex_count = u32::MAX)
    });
    corrupt("bad_local_index", &|b| {
        b[p.offset as usize + p.indices_offset as usize] = 255;
        rehash(b, h.pages_offset as usize);
    });
    corrupt("nonfinite_position", &|b| {
        b[p.offset as usize..p.offset as usize + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        rehash(b, h.pages_offset as usize);
    });
    corrupt("nonfinite_sphere", &|b| {
        patch::<Cluster>(b, h.clusters_offset as usize, |c| c.sphere[0] = f32::NAN)
    });
    corrupt("bad_cluster_page", &|b| {
        patch::<Cluster>(b, h.clusters_offset as usize, |c| c.page = u32::MAX)
    });
    corrupt("bad_cluster_vertices", &|b| {
        patch::<Cluster>(b, h.clusters_offset as usize, |c| {
            c.vertex_offset = u32::MAX
        })
    });
    corrupt("bad_refined_group", &|b| {
        patch::<Cluster>(b, h.clusters_offset as usize, |c| c.refined = c.group)
    });
    corrupt("bad_group_range", &|b| {
        patch::<Group>(b, h.groups_offset as usize, |g| g.first_cluster = u32::MAX)
    });
    corrupt("bad_group_depth", &|b| {
        patch::<Group>(b, h.groups_offset as usize, |g| g.depth = u32::MAX)
    });
    corrupt("bvh_cycle", &|b| {
        patch::<Node>(b, h.nodes_offset as usize, |n| {
            n.group = u32::MAX;
            n.child_offset = 0;
            n.child_count = 1;
        })
    });
    corrupt("bvh_invalid_group", &|b| {
        patch::<Node>(b, h.nodes_offset as usize, |n| {
            n.group = h.group_count;
            n.child_count = 0;
        })
    });
    corrupt("header_padding", &|b| b[size_of::<Header>()] = 1);
    for (pi, page) in reader.pages.iter().enumerate() {
        let vertex_end = page.vertex_count as usize * size_of::<clod_format::Vertex>();
        let index_end = page.indices_offset as usize + page.index_count as usize;
        for (name, at, end) in [
            ("vertex_padding", vertex_end, page.indices_offset as usize),
            ("index_padding", index_end, page.byte_length as usize),
        ] {
            if at < end {
                corrupt(name, &|b| {
                    b[page.offset as usize + at] = 1;
                    rehash(b, h.pages_offset as usize + pi * size_of::<Page>());
                });
            }
        }
    }
    corrupt("culling_sphere_excludes_vertices", &|b| {
        patch::<Cluster>(b, h.clusters_offset as usize, |c| c.sphere[3] = 0.0);
    });
    corrupt("ao_without_flag", &|b| {
        patch::<clod_format::Vertex>(b, p.offset as usize, |v| v.color = 0x7fff_ffff);
        rehash(b, h.pages_offset as usize);
    });
    corrupt("flag_without_ao", &|b| {
        patch::<Header>(b, 0, |h| h.flags = 2);
    });
    corrupt("color_without_flag", &|b| {
        patch::<clod_format::Vertex>(b, p.offset as usize, |v| v.color = 0);
        rehash(b, h.pages_offset as usize);
    });
    corrupt("flag_without_color", &|b| {
        patch::<Header>(b, 0, |h| h.flags = clod_format::HAS_COLOR)
    });
    let rewrite_bounds = |b: &mut Vec<u8>, gi: usize, bounds: clod_format::Bounds| {
        patch::<Group>(b, h.groups_offset as usize + gi * size_of::<Group>(), |g| {
            g.simplified = bounds
        });
        for (ci, c) in reader.clusters.iter().enumerate() {
            patch::<Cluster>(
                b,
                h.clusters_offset as usize + ci * size_of::<Cluster>(),
                |out| {
                    if c.group as usize == gi {
                        out.simplified = bounds;
                    }
                    if c.refined as usize == gi {
                        out.refined_bounds = bounds;
                    }
                },
            );
        }
        for (ni, n) in reader.nodes.iter().enumerate() {
            if n.group as usize == gi {
                patch::<Node>(b, h.nodes_offset as usize + ni * size_of::<Node>(), |n| {
                    n.bounds = bounds
                });
            }
        }
    };
    corrupt("unreferenced_nonterminal", &|b| {
        let gi = reader.groups.len() - 1;
        rewrite_bounds(
            b,
            gi,
            clod_format::Bounds {
                error: f32::MAX / 2.0,
                ..reader.groups[gi].simplified
            },
        );
    });
    corrupt("referenced_terminal", &|b| {
        for (gi, g) in reader.groups.iter().enumerate() {
            rewrite_bounds(
                b,
                gi,
                clod_format::Bounds {
                    error: f32::MAX,
                    ..g.simplified
                },
            );
        }
    });
    corrupt("ancestor_sphere_excludes_child", &|b| {
        let gi = reader.groups.len() - 1;
        rewrite_bounds(
            b,
            gi,
            clod_format::Bounds {
                radius: 0.0,
                ..reader.groups[gi].simplified
            },
        );
    });
    let mut triangle = clod_bake::Mesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let small = bake(&mut triangle, Config::default(), [0; 32]).unwrap();
    let small_reader = Reader::new(&small.bytes).unwrap();
    let page = small_reader.pages[0];
    for (name, at) in [
        ("small_vertex_padding", 60),
        (
            "small_index_padding",
            page.indices_offset + page.index_count,
        ),
    ] {
        let mut b = small.bytes.clone();
        b[page.offset as usize + at as usize] = 1;
        rehash(&mut b, small_reader.header.pages_offset as usize);
        cases.push((name.into(), b));
    }
    let mut failures = Vec::new();
    let mut rejected = 0;
    for (name, b) in &cases {
        match std::panic::catch_unwind(|| Reader::new(b).is_err()) {
            Ok(true) => rejected += 1,
            Ok(false) => failures.push(format!("accepted {name}")),
            Err(_) => failures.push(format!("panicked {name}")),
        }
    }
    let mut shifted = vec![0];
    shifted.extend_from_slice(&bytes);
    let unaligned = Reader::new(&shifted[1..]).is_err();
    if !unaligned {
        failures.push("accepted unaligned input".into());
    }
    println!(
        "{}",
        json!({"oracle":"reader_corruption","valid_bytes":bytes.len(),"cases":cases.len(),"rejected":rejected,"unaligned_rejected":unaligned,"failures":failures})
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("home"))
        .join("Library/Caches/exact2-cluster-lod/out")
        .join(format!("loader-tests-{}", std::process::id()))
}
#[test]
fn loader_formats_and_cli() {
    use base64::Engine;
    let dir = fixture_dir();
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let mesh = procedural::octasphere(0).expect("mesh");
    let mut cases: Vec<(&str, Vec<u8>, usize, usize, bool)> = Vec::new();
    let mut ply = Vec::new();
    procedural::write_ply(&mesh, &mut ply).expect("PLY");
    cases.push(("binary.ply", ply.clone(), 6, 8, false));
    let mut ascii = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        mesh.positions.len(),
        mesh.indices.len() / 3
    );
    for p in &mesh.positions {
        ascii.push_str(&format!("{} {} {} 0 0 1 12 34 56\n", p[0], p[1], p[2]));
    }
    for t in mesh.indices.chunks_exact(3) {
        ascii.push_str(&format!("3 {} {} {}\n", t[0], t[1], t[2]));
    }
    cases.push(("ascii.ply", ascii.into_bytes(), 6, 8, true));
    let mut binary_color=b"ply\nformat binary_big_endian 1.0\nelement vertex 6\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face 8\nproperty list uchar uint vertex_indices\nend_header\n".to_vec();
    for p in &mesh.positions {
        for v in p.iter().chain(&[0.0, 0.0, 1.0]) {
            binary_color.extend_from_slice(&v.to_be_bytes());
        }
        binary_color.extend_from_slice(&[12, 34, 56]);
    }
    for t in mesh.indices.chunks_exact(3) {
        binary_color.push(3);
        for v in t {
            binary_color.extend_from_slice(&v.to_be_bytes());
        }
    }
    cases.push(("color_big_endian.ply", binary_color, 6, 8, true));
    let mut obj = String::new();
    for p in &mesh.positions {
        obj.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
    }
    for t in mesh.indices.chunks_exact(3) {
        obj.push_str(&format!(
            "f {}/1 {}/2 {}/3\n",
            t[0] as i32 - 6,
            t[1] as i32 - 6,
            t[2] as i32 - 6
        ));
    }
    cases.push(("negative.obj", obj.into_bytes(), 6, 8, false));
    let mut stl = vec![0u8; 80];
    stl.extend_from_slice(&8u32.to_le_bytes());
    for t in mesh.indices.chunks_exact(3) {
        stl.extend_from_slice(&[0; 12]);
        for i in t {
            for v in mesh.positions[*i as usize] {
                stl.extend_from_slice(&v.to_le_bytes());
            }
        }
        stl.extend_from_slice(&[0; 2]);
    }
    cases.push(("weld.stl", stl, 6, 8, false));
    let mut bin = Vec::new();
    for p in &mesh.positions {
        for v in p {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    for i in &mesh.indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let mut gltf = json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":bin.len()}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":72},{"buffer":0,"byteOffset":72,"byteLength":96}],"accessors":[{"bufferView":0,"componentType":5126,"count":6,"type":"VEC3","min":[-2,-2,-2],"max":[2,2,2]},{"bufferView":1,"componentType":5125,"count":24,"type":"SCALAR"}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}]});
    let mut js = serde_json::to_vec(&gltf).expect("JSON");
    while !js.len().is_multiple_of(4) {
        js.push(b' ');
    }
    let length = 12 + 8 + js.len() + 8 + bin.len();
    let mut glb = Vec::new();
    for word in [0x46546c67, 2, length as u32, js.len() as u32, 0x4e4f534a] {
        glb.extend_from_slice(&word.to_le_bytes());
    }
    glb.extend_from_slice(&js);
    for word in [bin.len() as u32, 0x004e4942] {
        glb.extend_from_slice(&word.to_le_bytes());
    }
    glb.extend_from_slice(&bin);
    cases.push(("mesh.glb", glb, 6, 8, false));
    gltf["buffers"][0]["uri"] = json!(format!(
        "data:application/octet-stream;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bin)
    ));
    cases.push((
        "embedded.gltf",
        serde_json::to_vec(&gltf).expect("JSON"),
        6,
        8,
        false,
    ));
    std::fs::write(dir.join("buffer.bin"), &bin).expect("buffer");
    gltf["buffers"][0]["uri"] = json!("buffer.bin");
    cases.push((
        "external.gltf",
        serde_json::to_vec(&gltf).expect("JSON"),
        6,
        8,
        false,
    ));
    let mut failures = Vec::new();
    let mut vertices = 0;
    let mut triangles = 0;
    for (name, data, v, t, color) in &cases {
        let path = dir.join(name);
        std::fs::write(&path, data).expect("fixture write");
        match loaders::load(&path) {
            Ok((m, hash)) => {
                vertices += m.positions.len();
                triangles += m.indices.len() / 3;
                if m.positions.len() != *v
                    || m.indices.len() / 3 != *t
                    || m.colors.is_some() != *color
                    || hash != <[u8; 32]>::from(Sha256::digest(data))
                {
                    failures.push(format!("incorrect {name}"));
                }
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    let cli = env!("CARGO_BIN_EXE_clod-bake");
    let output = dir.join("fixture.clod");
    let input = dir.join("binary.ply");
    let cut = dir.join("cut.obj");
    let commands = vec![
        vec![input.display().to_string(), output.display().to_string()],
        vec!["--inspect".into(), output.display().to_string()],
        vec![
            "--cut".into(),
            output.display().to_string(),
            "--threshold".into(),
            "0".into(),
            "--obj".into(),
            cut.display().to_string(),
        ],
    ];
    for args in &commands {
        let mut child = std::process::Command::new(cli)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("CLI launch");
        println!(
            "{}",
            json!({"oracle":"cli_process","pid":child.id(),"arguments":args})
        );
        let stdout = child.stdout.take().expect("stdout");
        let value: std::result::Result<serde_json::Value, _> = serde_json::from_reader(stdout);
        let status = child.wait().expect("CLI wait");
        if !status.success() || value.is_err() {
            failures.push(format!("CLI {args:?}: {status}, {value:?}"));
        }
    }
    if let Ok((cut_mesh, _)) = loaders::load(&cut) {
        if cut_mesh.indices.len() != mesh.indices.len() {
            failures.push("CLI cut triangle count".into());
        }
    } else {
        failures.push("CLI cut did not load".into());
    }
    let mut rejected = 0;
    for (name, data) in [
        ("truncated.ply", &ply[..ply.len() - 1]),
        ("bad.obj", b"v 0 0 0\nf 1 2 3\n".as_slice()),
        ("truncated.stl", b"short".as_slice()),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, data).expect("bad fixture");
        if loaders::load(&path).is_err() {
            rejected += 1;
        } else {
            failures.push(format!("accepted {name}"));
        }
    }
    println!(
        "{}",
        json!({"oracle":"loaders","formats":cases.len(),"vertices":vertices,"triangles":triangles,"cli_commands":commands.len(),"malformed_rejected":rejected,"failures":failures})
    );
    std::fs::remove_dir_all(dir).expect("remove own fixtures");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn public_geometry_accessors_reject_overflow() {
    let mut mesh = procedural::octasphere(0).unwrap();
    let bytes = bake(&mut mesh, Config::default(), [0; 32]).unwrap().bytes;
    let original = Reader::new(&bytes).unwrap();
    let mut failures = Vec::new();
    for case in 0..5 {
        let mut pages = original.pages.to_vec();
        let mut clusters = original.clusters.to_vec();
        match case {
            0 => {
                pages[0].offset = u64::MAX;
                pages[0].byte_length = 2;
            }
            1 => pages[0].byte_length = u64::MAX,
            2 => {
                clusters[0].vertex_offset = u32::MAX;
                clusters[0].vertex_count = 2;
            }
            3 => {
                clusters[0].triangle_offset = u32::MAX;
                clusters[0].triangle_count = u32::MAX;
            }
            _ => clusters[0].page = u32::MAX,
        }
        let mut reader = Reader::new(&bytes).unwrap();
        reader.pages = &pages;
        reader.clusters = &clusters;
        let rejected = matches!(
            std::panic::catch_unwind(|| reader.geometry(0).is_none()),
            Ok(true)
        );
        println!("accessor_overflow case={case} rejected={rejected}");
        if !rejected {
            failures.push(case);
        }
    }
    println!("accessor_cases=5 failures={failures:?}");
    assert!(failures.is_empty());
}
