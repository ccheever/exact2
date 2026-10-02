//! CSS inheritance across the Apple host's batch (LLP 1035.000 slice 1): a
//! run or an editor carries the computed rows it measures with; an
//! ancestor's change re-sends exactly the descendants that follow it; and
//! `layout <node>`'s runner half (LLP 1035.002 D1) says where each value
//! came from.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn count(batch: &str, op: &str) -> usize {
    batch.matches(&format!("\"op\":\"{op}\"")).count()
}

fn op(batch: &str, id: u32) -> String {
    batch
        .split("{\"op\":")
        .find(|part| part.contains(&format!("\"id\":{id},")))
        .unwrap_or_else(|| panic!("missing {id} in {batch}"))
        .to_owned()
}

const SRC: &str = r##"component Type
  state big = false
  action toggle writes big
    big = not big
  view
    column font-size=(big ? 24 : 20) line-height="24px" testId="root"
      button press=toggle testId="toggle"
        text "Toggle"
      text testId="paragraph"
        text "plain" testId="plain"
        text "bold" font-weight=700 testId="bold"
        text "small" font-size=12 testId="small"
      input testId="field" value="Input"
      column font-size=14
        text "nested" testId="nested"
"##;

#[test]
fn symbol_identity_and_inherited_font_cross_the_image_boundary() {
    let plan = contract::compile(r##"component App
  state large = false
  action change writes large
    large = not large
  view
    column font-size=(large ? 28 : 17) font-weight=(large ? 600 : 400)
      button "Change" press=change testId="change"
      image (large ? "symbol:send" : "symbol:search") tint-color="light-dark(#007aff,#0a84ff)" testId="symbol"
"##).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let image = view(&host, "symbol");
    let initial = op(&first, image);
    assert!(initial.contains("\"symbolName\":\"magnifyingglass\""));
    assert!(initial.contains("\"font_size\":17"));
    assert!(initial.contains("\"font_weight\":400"));
    assert!(
        initial.contains("\"tint_color\":[[0,122,255,255],[10,132,255,255]]"),
        "{initial}"
    );
    let changed = host.dispatch(view(&host, "change"), Event::Press);
    assert!(changed.contains("\"symbolName\":\"arrow.up\""), "{changed}");
    assert!(changed.contains("\"font_size\":28"), "{changed}");
    assert!(changed.contains("\"font_weight\":600"), "{changed}");
}

#[test]
fn raw_symbol_names_cross_the_boundary_without_a_catalog() {
    let plan = contract::compile(
        r#"component App
  state source = "symbol:sf/airpodsmax"
  action missing writes source
    source = "symbol:sf/exact.nonexistent"
  action empty writes source
    source = "symbol:sf/"
  action restore writes source
    source = "symbol:sf/airpodsmax"
  view
    column
      image source testId="icon"
      button "Missing" press=missing testId="missing"
      button "Empty" press=empty testId="empty"
      button "Restore" press=restore testId="restore"
"#,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.,
        874.,
    )
    .unwrap();
    assert!(first.contains("\"symbolName\":\"airpodsmax\""));
    for (action, expected) in [
        ("missing", "exact.nonexistent"),
        ("empty", ""),
        ("restore", "airpodsmax"),
    ] {
        let batch = host.dispatch(view(&host, action), Event::Press);
        assert!(
            batch.contains(&format!("\"symbolName\":\"{expected}\"")),
            "{batch}"
        );
    }
}

#[test]
fn inherited_text_rows_reach_runs_and_editors_as_computed_values() {
    let plan = contract::compile(SRC).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let plain = view(&host, "plain");
    let bold = view(&host, "bold");
    let small = view(&host, "small");
    let field = view(&host, "field");
    let nested = view(&host, "nested");
    // A run or an editor carries the computed rows it measures with; an own
    // row stays its own; a box node carries only its colour.
    for id in [plain, bold, field] {
        assert!(
            op(&first, id).contains("\"font_size\":20"),
            "{}",
            op(&first, id)
        );
    }
    assert!(op(&first, plain).contains("\"line_height\":\"24px\""));
    assert!(op(&first, bold).contains("\"font_weight\":700"));
    assert!(op(&first, small).contains("\"font_size\":12"));
    assert!(op(&first, small).contains("\"line_height\":\"24px\""));
    assert!(op(&first, nested).contains("\"font_size\":14"));
    assert!(!op(&first, view(&host, "toggle")).contains("font_size"));
    // The ancestor's change re-sends exactly the descendants that follow it.
    let changed = host.dispatch_at(view(&host, "toggle"), Event::Press, 0.0);
    for id in [plain, bold, field] {
        assert!(
            op(&changed, id).contains("\"font_size\":24"),
            "{}",
            op(&changed, id)
        );
    }
    for id in [small, nested] {
        assert!(
            !changed.contains(&format!("\"op\":\"style\",\"id\":{id},")),
            "{id} overrides the row and must not be re-sent: {changed}"
        );
    }
    assert_eq!(count(&host.resize(402.0, 874.0), "style"), 0);
}

#[test]
fn a_node_read_names_where_each_value_came_from() {
    let plan = contract::compile(SRC).unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let root = view(&host, "root");
    let small = view(&host, "small");
    let reply = host.agent(&format!("{{\"op\":\"node\",\"id\":{small}}}"));
    assert!(
        reply.contains("\"epoch\":") && reply.contains("\"incarnation\":1"),
        "{reply}"
    );
    // Its own size is authored; the line height is its paragraph's parent's
    // — the root column — by inheritance; letter spacing is initial.
    assert!(
        reply.contains("\"font_size\":{\"value\":12,\"source\":\"authored\"}"),
        "{reply}"
    );
    assert!(
        reply.contains(&format!(
            "\"line_height\":{{\"value\":\"24px\",\"source\":\"inherited\",\"from\":{root}}}"
        )),
        "{reply}"
    );
    assert!(
        reply.contains("\"letter_spacing\":{\"value\":0,\"source\":\"initial\"}"),
        "{reply}"
    );
    assert!(
        reply.contains("\"text_color\":{\"value\":\"#000000\",\"source\":\"initial\"}"),
        "{reply}"
    );
    // A box row never appears unless authored.
    assert!(!reply.contains("\"width\":"), "{reply}");
    assert!(
        reply.contains("\"props\":{\"text\":\"small\",\"testId\":\"small\"}")
            || reply.contains("\"testId\":\"small\""),
        "{reply}"
    );
    assert!(
        reply.contains("\"site\":") && reply.contains("\"frame\":{"),
        "{reply}"
    );
    // A stale id is refused by name, never answered from a reused slot.
    let stale = host.agent("{\"op\":\"node\",\"id\":9999}");
    assert!(stale.contains("stale node #9999"), "{stale}");
}

#[test]
fn caret_color_reaches_editors_and_explicit_auto_stops_inheritance() {
    let plan = contract::compile(
        r##"component Caret
  state changed = false
  action toggle writes changed
    changed = not changed
  view
    column caret-color=(changed ? "#00aaff" : "#ffffff")
      button press=toggle testId="toggle"
        text "Change caret"
      input testId="inherited" value="Input"
      textarea testId="auto" value="Default" caret-color="auto"
      textarea testId="transparent" value="No caret" caret-color="#00000000"
"##,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    assert!(op(&first, view(&host, "inherited")).contains("\"caret_color\":[255,255,255,255]"));
    assert!(!op(&first, view(&host, "auto")).contains("[255,255,255,255]"));
    assert!(op(&first, view(&host, "transparent")).contains("\"caret_color\":[0,0,0,0]"));
    let changed = host.dispatch_at(view(&host, "toggle"), Event::Press, 0.0);
    assert!(op(&changed, view(&host, "inherited")).contains("\"caret_color\":[0,170,255,255]"));
    for name in ["auto", "transparent"] {
        assert!(!changed.contains(&format!("\"op\":\"style\",\"id\":{},", view(&host, name))));
    }
}

#[test]
fn border_layout_paint_and_current_color_follow_live_style_changes() {
    let plan = contract::compile(r##"component Borders
  state mode = "none"
  state blue = false
  action setMode(value: string) writes mode
    mode = value
  action recolor writes blue
    blue = not blue
  view
    column color=(blue ? "#0000ff" : "#ff0000")
      button "Solid" press=setMode("solid") testId="solid"
      button "None" press=setMode("none") testId="none"
      button "Hidden" press=setMode("hidden") testId="hidden"
      button "Color" press=recolor testId="color"
      box testId="wide" width=100 height=60 padding=10 border-width=8 border-style=mode
      box testId="default" width=100 height=60 padding=10 border-style=mode
      box testId="transparent" width=100 height=60 padding=10 border-width=8 border-style=mode border-color="#00000000"
"##).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let wide = view(&host, "wide");
    let default = view(&host, "default");
    let transparent = view(&host, "transparent");
    let size = |host: &Host<NoData>, id| {
        let frame = host.runner().kernel().node(id).unwrap().frame;
        (frame.width, frame.height)
    };
    assert_eq!(size(&host, wide), (120.0, 80.0));
    assert!(op(&first, wide).contains("\"border_width_top\":0"));
    let solid = host.dispatch(view(&host, "solid"), Event::Press);
    assert_eq!(size(&host, wide), (136.0, 96.0));
    assert_eq!(size(&host, default), (126.0, 86.0));
    assert_eq!(size(&host, transparent), (136.0, 96.0));
    assert!(op(&solid, wide).contains("\"border_color_top\":[255,0,0,255]"));
    assert!(op(&solid, default).contains("\"border_width_top\":3"));
    assert!(op(&solid, transparent).contains("\"border_color_top\":[0,0,0,0]"));
    let blue = host.dispatch(view(&host, "color"), Event::Press);
    assert!(op(&blue, wide).contains("\"border_color_top\":[0,0,255,255]"));
    assert_eq!(size(&host, wide), (136.0, 96.0));
    for control in ["hidden", "none"] {
        let batch = host.dispatch(view(&host, control), Event::Press);
        assert_eq!(size(&host, wide), (120.0, 80.0));
        assert_eq!(size(&host, default), (120.0, 80.0));
        if control == "hidden" {
            assert!(op(&batch, wide).contains("\"border_width_top\":0"));
        } else {
            // `hidden` to `none` paints the same box: the presenter gets no
            // style (it never reads `border-style`, the widths say it).
            assert_eq!(count(&batch, "style"), 0, "{batch}");
        }
    }
}

#[test]
fn apple_artifacts_own_paths_locks_identity_and_failed_placement() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = std::process::Command::new("bun")
        .current_dir(root)
        .args(["--input-type=module", "-e", r#"
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {resolveApp, bakeOutput, claimBuildOutput, cargoLibraryTarget, appleCargoClaims} from './scripts/app.mjs';
import {appleArtifacts, appleBuildLock, placeAppleArtifact, assertAppleIdentity, captureAppleProduct} from './host/apple/build.mjs';
const run = mkdtempSync(resolve(tmpdir(), 'exact-apple-ownership-'));
try {
  const apps = ['one', 'two'].map(name => {
    const dir = resolve(run, name); mkdirSync(dir); writeFileSync(resolve(dir, 'app.contract'), '');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({name:'Same',app:{id:'org.example.same',name:'Same'}}));
    process.env.EXACT_APP_DIR = dir; process.env.CARGO_TARGET_DIR = resolve(run, 'target');
    return resolveApp(name);
  });
  const [a,b] = apps, first = appleArtifacts(a);
  assert.notEqual(first.owner, appleArtifacts(b).owner);
  assert.notEqual(bakeOutput(a), bakeOutput(b));
  symlinkSync(a.dir, resolve(run, 'alias')); process.env.EXACT_APP_DIR = resolve(run, 'alias');
  assert.equal(first.owner, appleArtifacts(resolveApp('one')).owner);
  const forms = ['macos', 'ios-simulator', 'ios'].flatMap(destination =>
    ['embedded','updating'].flatMap(composition => ['development','production'].flatMap(trust =>
      [false,true].map(host => appleArtifacts(a,{destination,composition,trust,host})))));
  for (const key of ['products','binary']) assert.equal(new Set(forms.map(p => p[key])).size, forms.length, key);
  for (const key of ['scratch','embed']) assert.equal(new Set(forms.map(p => p[key])).size, forms.length / 2, key);
  assert.equal(new Set(forms.map(p => p.lock)).size, 1);
  assert.equal(appleArtifacts(a,{trust:'production'}).bundle, first.bundle);
  const release = appleBuildLock(a);
  const contender = spawnSync(process.execPath,['--input-type=module','-e',
    `import {appleBuildLock} from './host/apple/build.mjs'; import {resolveApp} from './scripts/app.mjs'; appleBuildLock(resolveApp('one'));`], {encoding:'utf8',env:{...process.env}});
  assert.notEqual(contender.status, 0); assert.match(contender.stderr, /Apple build busy/);
  assert.match(contender.stderr, new RegExp(String(process.pid))); release();
  assert.ok(!existsSync(first.lock)); appleBuildLock(a)();
  const named = resolve(a.target, '.apple-cargo-locks', 'target', 'same-apple.lock');
  const releaseNamed = claimBuildOutput(a, named); assert.throws(()=>claimBuildOutput(b, named), /Apple build busy/); releaseNamed();
  // Two distinct packages really emit the same named archive. The claim
  // follows Cargo metadata's selected library target, through capture.
  const libraries = apps.map((app, i) => {
    const name = ['alpha-beta-apple','alpha_beta-apple'][i];
    writeFileSync(resolve(app.dir,'Cargo.toml'), `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2021"\n[lib]\npath = "lib.rs"\ncrate-type = ["staticlib"]\n`);
    writeFileSync(resolve(app.dir,'lib.rs'), `#[no_mangle] pub extern "C" fn alpha() -> u32 { ${i} }`);
    const metadata = spawnSync('cargo',['metadata','--no-deps','--format-version','1','--manifest-path',resolve(app.dir,'Cargo.toml')],{encoding:'utf8',env:{...process.env}});
    assert.equal(metadata.status,0,metadata.stderr);
    return cargoLibraryTarget(JSON.parse(metadata.stdout).packages[0]);
  });
  const firstClaims = appleCargoClaims(a,'host',[libraries[0]]);
  assert.deepEqual(firstClaims,appleCargoClaims(b,'host',[libraries[1]]));
  assert.equal(appleCargoClaims(a,'host',[...libraries,{name:'alpha-beta-apple'}]).length,1);
  const buildLibrary = app => {
    const built = spawnSync('cargo',['build','--manifest-path',resolve(app.dir,'Cargo.toml'),'--message-format=json'],{encoding:'utf8',env:{...process.env}});
    assert.equal(built.status,0,built.stderr);
    return built.stdout.trim().split('\n').map(line=>JSON.parse(line)).find(m=>m.reason==='compiler-artifact').filenames.find(p=>p.endsWith('.a'));
  };
  const releaseLibrary = claimBuildOutput(a,firstClaims[0]);
  const library = buildLibrary(a), bytesBefore = readFileSync(library);
  const aliasContender = spawnSync(process.execPath,['--input-type=module','-e',
    `import {claimBuildOutput,appleCargoClaims} from './scripts/app.mjs'; const app=${JSON.stringify(b)}; claimBuildOutput(app,appleCargoClaims(app,'host',${JSON.stringify([libraries[1]])})[0]);`],{encoding:'utf8',env:{...process.env}});
  assert.notEqual(aliasContender.status,0); assert.match(aliasContender.stderr,/Apple build busy/);
  assert.deepEqual(readFileSync(library),bytesBefore); releaseLibrary();
  const releaseOther = claimBuildOutput(b,firstClaims[0]);
  assert.equal(buildLibrary(b),library); assert.notDeepEqual(readFileSync(library),bytesBefore); releaseOther();
  const placed = resolve(run,'placed'), next = resolve(run,'next'); mkdirSync(placed);
  writeFileSync(resolve(placed,'ExactMac'),'previous');
  assert.throws(()=>placeAppleArtifact(next,placed),/ENOENT/);
  assert.equal(readFileSync(resolve(placed,'ExactMac'),'utf8'),'previous');
  mkdirSync(next); writeFileSync(resolve(next,'ExactMac'),'new'); placeAppleArtifact(next,placed);
  assert.equal(readFileSync(resolve(placed,'ExactMac'),'utf8'),'new');
  const {createHash} = await import('node:crypto');
  const source = resolve(run,'libsame.a'), captured = resolve(run,'capture.a');
  const bytes = Buffer.from('first checkout'); writeFileSync(source,bytes);
  const build = {products:[{path:source,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')}]};
  captureAppleProduct(build,source,captured); assert.deepEqual(readFileSync(captured),bytes);
  writeFileSync(source,'second checkout');
  assert.throws(()=>captureAppleProduct(build,source,captured),/changed before Apple capture/);
  assert.deepEqual(readFileSync(captured),bytes);
  const binary = resolve(placed,'ExactMac'), compat = 'a'.repeat(32);
  const embedded = id => JSON.stringify({id:compat,inputs:{abi:{c:4},app:id}});
  writeFileSync(binary, embedded(a.id)); assertAppleIdentity(a,binary,compat);
  writeFileSync(binary, embedded('org.foreign.app') + a.id);
  assert.throws(()=>assertAppleIdentity(a,binary), /embedded app identity/);
  writeFileSync(binary, a.id); assert.throws(()=>assertAppleIdentity(a,binary), /missing/);
} finally { rmSync(run,{recursive:true,force:true}); }
"#])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn a_reloaded_plan_resolves_line_height_kinds_against_the_new_receiving_font() {
    let source = |height: &str, size| {
        format!("component App\n  view\n    column font-size=16 line-height={height}\n      text \"child\" font-size={size} testId=\"child\"\n")
    };
    let plan = contract::compile(&source("1.5", 20)).unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        800.0,
    )
    .unwrap();
    assert_eq!(
        host.runner()
            .kernel()
            .node(view(&host, "child"))
            .unwrap()
            .text_style()
            .line_height,
        Some(30.0)
    );
    let carry = host.carry();
    for (height, size, expected) in [
        ("1.5", 24, Some(36.0)),
        ("\"24px\"", 40, Some(24.0)),
        ("\"normal\"", 20, None),
        ("0", 20, Some(0.0)),
    ] {
        let plan = contract::compile(&source(height, size)).unwrap();
        let (reloaded, _) = Host::boot_with(
            &plan.encode(),
            NoData,
            Box::new(MonospaceMeasurer::default()),
            400.0,
            800.0,
            Some(&carry),
        )
        .unwrap();
        assert_eq!(
            reloaded
                .runner()
                .kernel()
                .node(view(&reloaded, "child"))
                .unwrap()
                .text_style()
                .line_height,
            expected
        );
    }
}

#[test]
fn paragraph_batches_preserve_inline_identity_and_replace_the_complete_run_table() {
    let plan = contract::compile(r##"component Inline
  state changed = false
  resource additions = additions(changed) as shape list<string>
  action change writes changed
    changed = not changed
  view
    column
      button "Change" press=change testId="change"
      text testId="paragraph"
        text (changed ? "after 👩‍🚀" : "before é") testId="run" press=change font-weight=(changed ? 700 : 400) color=(changed ? "#ff0000" : "#000000")
        text testId="nested"
          text "link" href="https://example.invalid/" testId="link"
        each value in additions key=value
          text value testId="added"
"##).unwrap();
    struct InlineData;
    impl DataSource for InlineData {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(if args == [Value::Bool(true)] {
                vec![Value::str("added")]
            } else {
                vec![]
            }))
        }
    }
    let (mut host, first) = Host::boot(
        &plan.encode(),
        InlineData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        800.0,
    )
    .unwrap();
    let paragraph = view(&host, "paragraph");
    let run = view(&host, "run");
    let nested = view(&host, "nested");
    let link = view(&host, "link");
    assert!(count(&first, "paragraph") >= 1);
    for id in [run, nested, link] {
        for kind in ["create", "props", "style", "children", "frame", "destroy"] {
            assert!(
                !first.contains(&format!("\"op\":\"{kind}\",\"id\":{id},")),
                "{first}"
            );
        }
    }
    assert!(first.contains(&format!(
        "\"op\":\"paragraph\",\"id\":{paragraph},\"runs\":["
    )));
    assert!(first.contains("\"handlers\":[\"press\"]"));
    assert!(first.contains("https://example.invalid/"));
    let tree = host.agent("{\"op\":\"tree\"}");
    assert!(
        tree.contains("before é") && tree.contains("\"testId\":\"run\""),
        "{tree}"
    );
    // The logical run dispatches into the same action without a native view.
    let changed = host.dispatch(run, Event::Press);
    assert_eq!(view(&host, "run"), run);
    assert_eq!(count(&changed, "paragraph"), 1, "{changed}");
    assert!(changed.contains("after 👩‍🚀") && changed.contains("\"font_weight\":700"));
    assert!(changed.contains("\"testId\":\"added\""));
    let added = view(&host, "added");
    let removed = host.dispatch(run, Event::Press);
    assert_eq!(count(&removed, "paragraph"), 1);
    assert!(!removed.contains("\"testId\":\"added\""));
    assert!(!removed.contains(&format!("\"op\":\"destroy\",\"id\":{added}}}")));
    assert_eq!(count(&host.resize(420.0, 800.0), "paragraph"), 0);
}

/// LLP 1064: Swift paints a run from its `text` prop, so the prop crosses as
/// the string the kernel measured — `text-transform` applied, a word split
/// across runs one word, a field's value as typed — and an ancestor's change
/// re-sends it. `box-shadow` crosses as its four rows.
#[test]
fn text_transform_crosses_as_the_measured_string_and_box_shadow_as_its_rows() {
    let plan = contract::compile(
        r##"component Case
  state caps = false
  action toggle writes caps
    caps = not caps
  view
    column text-transform=(caps ? "uppercase" : "capitalize") box-shadow="0 2px 12px rgba(0, 0, 0, 0.2)" testId="root"
      button press=toggle testId="toggle"
        text "toggle"
      text "straße here" testId="own"
      text testId="paragraph"
        text "hel" testId="a"
        text "lo world" testId="b"
      input value="typed" testId="field"
"##,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    assert!(
        op(&first, view(&host, "own")).contains("\"text\":\"Straße Here\""),
        "{first}"
    );
    assert!(
        first.contains("\"text\":\"Hel\"") && first.contains("\"text\":\"lo World\""),
        "{first}"
    );
    assert!(op(&first, view(&host, "field")).contains("\"value\":\"typed\""));
    let root = op(&first, view(&host, "root"));
    for row in [
        "\"shadow_color\":[0,0,0,51]",
        "\"shadow_offset\":[0,2]",
        "\"shadow_radius\":12",
        "\"shadow_opacity\":1",
    ] {
        assert!(root.contains(row), "{row} in {root}");
    }
    let changed = host.dispatch_at(view(&host, "toggle"), Event::Press, 0.0);
    for text in ["STRASSE HERE", "HEL", "LO WORLD"] {
        assert!(
            changed.contains(&format!("\"text\":\"{text}\"")),
            "{text}: {changed}"
        );
    }
    assert!(!changed.contains("\"value\":\"TYPED\""), "{changed}");
}
