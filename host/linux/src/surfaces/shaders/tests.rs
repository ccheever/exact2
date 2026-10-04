use super::*;
use serde_json::json;

fn shader_fixture() -> (std::path::PathBuf, Value) {
    let (path, mut compat) = super::super::tests::fixture();
    let source = path.with_file_name("probe.c");
    let mut text = std::fs::read_to_string(&source).unwrap();
    text.push_str(
        r#"
static char shader_calls[256];
static uint32_t shader_at;
uint32_t gpu_shader_validate(const unsigned char *n, size_t nl, const unsigned char *s, size_t sl) {
  shader_calls[shader_at++]='v';
  return nl!=1 || (n[0]!='a' && n[0]!='b') || (sl>=6 && !memcmp(s,"reject",6));
}
void gpu_shaders_clear(void) { shader_calls[shader_at++]='c'; }
uint32_t gpu_shader(const unsigned char *n, size_t nl, const unsigned char *s, size_t sl) {
  shader_calls[shader_at++]='s'; return sl>=4 && !memcmp(s,"fail",4);
}
const char *test_shader_calls(void) { return shader_calls; }
"#,
    );
    std::fs::write(&source, text).unwrap();
    super::super::tests::compile_fixture(&source, &path);
    compat["embedded"]["gpu"]["sha256"] =
        format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap())).into();
    compat["inputs"]["gpuSurfaces"] = json!([{"name":"a"},{"name":"b"}]);
    (path, compat)
}

fn selected(a: &'static [u8], b: &'static [u8]) -> Assets {
    Assets::selected(
        Default::default(),
        Arc::new(move |name| {
            Ok(match name {
                "shaders/a.wgsl" => Some(Arc::from(a)),
                "shaders/b.wgsl" => Some(Arc::from(b)),
                _ => None,
            })
        }),
    )
}

fn calls(abi: &Abi) -> String {
    unsafe {
        let ptr =
            abi.symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(b"test_shader_calls")();
        std::ffi::CStr::from_ptr(ptr).to_str().unwrap().into()
    }
}

#[test]
fn validates_complete_pack_before_replacement_and_unchanged_pack_does_no_gpu_work() {
    let (path, compat) = shader_fixture();
    let mut abi = Abi::open_path(&path, &compat, "").unwrap();
    abi.rendered = true; // Test the loader ABI without requesting a GPU device.
    let pack = abi
        .prepare_shaders(&compat, &selected(b"old-a", b"old-b"))
        .unwrap();
    assert_eq!(calls(&abi), "vv");
    assert!(abi.shaders.is_empty());
    abi.commit_shaders(pack).unwrap();
    assert_eq!(calls(&abi), "vvcss");
    let old = abi.shaders.clone();
    assert!(abi
        .prepare_shaders(&compat, &selected(b"old-a", b"old-b"))
        .unwrap()
        .is_none());
    assert_eq!(calls(&abi), "vvcss");
    // The first candidate source is valid; the second refuses. No clear or
    // registration may occur, and the current complete pack stays live.
    assert!(abi
        .prepare_shaders(&compat, &selected(b"new-a", b"reject-interface"))
        .unwrap_err()
        .contains("`b`"));
    assert_eq!(calls(&abi), "vvcssvv");
    assert_eq!(abi.shaders, old);
    let pack = abi
        .prepare_shaders(&compat, &selected(b"new-a", b"new-b"))
        .unwrap();
    abi.commit_shaders(pack).unwrap();
    assert_eq!(calls(&abi), "vvcssvvvvcss");
    assert_eq!(&*abi.shaders["a"], b"new-a");
    // An integrity refusal is checked even when the accepted names are unchanged.
    let corrupt = Assets::selected(
        Default::default(),
        Arc::new(|_| Err("signed digest mismatch".into())),
    );
    assert!(abi
        .prepare_shaders(&compat, &corrupt)
        .unwrap_err()
        .contains("signed digest"));
    assert_eq!(calls(&abi), "vvcssvvvvcss");
    let mut unknown = compat.clone();
    unknown["inputs"]["gpuSurfaces"][1]["name"] = "unexpected".into();
    let extra = Assets::selected(
        Default::default(),
        Arc::new(|_| Ok(Some(Arc::from(b"source".as_slice())))),
    );
    assert!(abi
        .prepare_shaders(&unknown, &extra)
        .unwrap_err()
        .contains("unexpected"));
    drop(abi);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn headless_shaderful_module_needs_neither_sources_nor_shader_symbols() {
    let (path, mut compat) = super::super::tests::fixture();
    compat["inputs"]["gpuSurfaces"] = json!([{"name":"never-read"}]);
    let mut abi = Abi::open_path(&path, &compat, "").unwrap();
    let assets = Assets::selected(
        Default::default(),
        Arc::new(|_| panic!("headless must not read")),
    );
    assert!(abi.prepare_shaders(&compat, &assets).unwrap().is_none());
    abi.commit_shaders(None).unwrap();
    assert_eq!(
        unsafe {
            abi.symbol::<unsafe extern "C" fn(u32, *const u8, usize, f64) -> u32>(b"gpu_bind_at")(
                1,
                b"[]".as_ptr(),
                2,
                0.,
            )
        },
        0
    );
    drop(abi);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn shader_sources_enforce_roster_integrity_utf8_bounds_and_selected_tombstones() {
    let root = std::env::temp_dir().join(format!("exact-shader-files-{}", std::process::id()));
    std::fs::create_dir_all(root.join("shaders")).unwrap();
    let path = root.join("shaders/a.wgsl");
    std::fs::write(&path, b"valid").unwrap();
    let mut compat = json!({"inputs":{"gpuSurfaces":[{"name":"a"}]},"embedded":{"assets":[{
        "name":"shaders/a.wgsl","bytes":5,"sha256":format!("{:x}",Sha256::digest(b"valid"))
    }]}});
    let assets = Assets::embedded(root.clone());
    assert_eq!(&*read_pack(&compat, &assets).unwrap()["a"], b"valid");
    std::fs::write(&path, b"other").unwrap();
    assert!(read_pack(&compat, &assets)
        .unwrap_err()
        .contains("digest mismatch"));
    // Selected source has its own integrity authority, not entry zero's hash.
    assert_eq!(
        &*read_pack(&compat, &selected(b"changed", b"")).unwrap()["a"],
        b"changed"
    );
    let absent = Assets::selected(root.clone(), Arc::new(|_| Ok(None)));
    assert!(read_pack(&compat, &absent)
        .unwrap_err()
        .contains("missing source"));
    assert!(read_pack(&compat, &selected(b"\xff", b""))
        .unwrap_err()
        .contains("UTF-8"));
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(LIMIT as u64 + 1).unwrap();
    drop(file);
    assert!(read_pack(&compat, &assets).unwrap_err().contains("256 MiB"));
    compat["inputs"]["gpuSurfaces"] = json!([{"name":"a"},{"name":"a"}]);
    assert!(read_pack(&compat, &selected(b"a", b"b"))
        .unwrap_err()
        .contains("duplicate"));
    compat["inputs"]["gpuSurfaces"] = json!([{"name":"../a"}]);
    assert!(read_pack(&compat, &assets).unwrap_err().contains("invalid"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn shader_and_store_acceptance_roll_back_without_advancing_either_owner() {
    let (path, compat) = shader_fixture();
    let mut abi = Abi::open_path(&path, &compat, "").unwrap();
    abi.rendered = true;
    let pack = abi
        .prepare_shaders(&compat, &selected(b"old-a", b"old-b"))
        .unwrap();
    abi.commit_shaders(pack).unwrap();
    let old = abi.shaders.clone();
    let mut surfaces = Surfaces::default();
    surfaces.abis.insert(String::new(), abi);
    let pack = surfaces.abis[""]
        .prepare_shaders(&compat, &selected(b"new-a", b"fail-registration"))
        .unwrap();
    assert!(surfaces
        .activate_shaders(pack, || panic!(
            "registration failure cannot reach the store"
        ))
        .is_err());
    assert_eq!(surfaces.abis[""].shaders, old);
    assert!(calls(&surfaces.abis[""]).ends_with("vvcsscss"));
    let pack = surfaces.abis[""]
        .prepare_shaders(&compat, &selected(b"new-a", b"new-b"))
        .unwrap();
    assert_eq!(
        surfaces.activate_shaders(pack, || Err("store refused".into())),
        Err("store refused".into())
    );
    assert_eq!(surfaces.abis[""].shaders, old);
    assert!(calls(&surfaces.abis[""]).ends_with("vvcsscss"));
    drop(surfaces);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
