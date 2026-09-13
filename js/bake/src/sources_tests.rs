use super::{sources, Scratch};

#[test]
fn captures_fonts_and_complete_static_trees_without_following_links() {
    let app = Scratch::new(&std::env::temp_dir()).unwrap();
    let inputs = [
        ("app.ts", &b"export const appId = 'test.assets';"[..]),
        ("fonts/face.ttf", &b"font bytes"[..]),
        ("fonts/face.otf", &b"other font bytes"[..]),
        ("assets/icons/menu.svg", &b"<svg/>"[..]),
        ("assets/icons/raster/menu.png", &b"\x89PNG\x00\xff"[..]),
        ("assets/data.bin", &b"\x00\x01\xfe\xff"[..]),
        ("deck/index.html", &b"<!doctype html>"[..]),
        ("gpu/shaders/paint.wgsl", &b"@fragment fn paint() {}"[..]),
    ];
    for (name, bytes) in inputs {
        let path = app.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    std::fs::write(app.0.join("private.bin"), b"not an app asset").unwrap();
    let captured = sources(&app.0).unwrap();
    assert_eq!(captured.len(), inputs.len());
    for (name, bytes) in inputs {
        assert_eq!(captured[std::path::Path::new(name)], bytes);
    }
    std::fs::write(app.0.join("assets/icons/menu.svg"), b"<svg>changed</svg>").unwrap();
    assert_ne!(sources(&app.0).unwrap(), captured);
    std::fs::remove_file(app.0.join("assets/data.bin")).unwrap();
    assert!(!sources(&app.0)
        .unwrap()
        .contains_key(std::path::Path::new("assets/data.bin")));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(app.0.join("private.bin"), app.0.join("assets/link.bin"))
            .unwrap();
        assert!(sources(&app.0).unwrap_err().contains("source links"));
    }
}
