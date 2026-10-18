//! The binding core on the Linux host (LLP 1121 D12, stage 5): what
//! Android's `ExactTamber(view)` asks by `Native.tamber`, a colour as the
//! painter draws it for a place, and the `inputs` its `observe` compares.

use exact_linux::presenter::{PainterChoice, Presenter};
use exact_runner::design;
use serde_json::{json, Value};

const DESIGN: &str = "tamber t\n\naxes\n  tone = plain | bold\n\ntheme\n  ink = #111111\n    when tone=bold: #ff0000\n  page = light-dark(#ffffff, #000000)\n";
const VIEW: &str = "component App\n  view\n    column width=200 height=100 color=\"ink\" background-color=\"page\"\n      text \"a\" testId=\"outer\"\n      column -exact-mode=\"tone=bold\" color=\"ink\"\n        text \"b\" testId=\"inner\"\n      column color-scheme=\"dark\" background-color=\"page\"\n        text \"c\" testId=\"night\"\n";

fn plan(dir: &std::path::Path, design: &str) -> exact_plan::Plan {
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), VIEW).unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap()
}

fn id(p: &Presenter<()>, test_id: &str) -> u32 {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn ask(p: &mut Presenter<()>, request: Value) -> Value {
    serde_json::from_str(&p.host_mut().tamber_json(&request.to_string())).unwrap()
}

/// `ink` at `view`, and the inputs there.
fn ink(p: &mut Presenter<()>, view: u32) -> (i64, String) {
    let a = ask(p, json!({"view": view, "names": ["ink"]}));
    let inputs = a["inputs"].as_str().unwrap().to_string();
    (a["colors"]["ink"].as_i64().unwrap(), inputs)
}

fn argb(hex: u32) -> i64 {
    i64::from(hex as i32)
}

#[test]
fn a_view_asks_its_colours_by_name() {
    let dir = std::env::temp_dir().join(format!("exact-linux-tamber-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = plan(&dir, DESIGN);
    let b = plan(&dir, &DESIGN.replace("#ff0000", "#00ff00"));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut p, err) = Presenter::boot_with(
        &a.encode(),
        (),
        (200., 100.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let (outer, inner, night) = (id(&p, "outer"), id(&p, "inner"), id(&p, "night"));

    // Two places under different pins answer different ints; the root is the window's.
    let (plain, outer_inputs) = ink(&mut p, outer);
    let (bold, inner_inputs) = ink(&mut p, inner);
    assert_eq!(plain, argb(0xff111111));
    assert_eq!(bold, argb(0xffff0000));
    assert_eq!(ink(&mut p, 0).0, plain);
    // A `color-scheme` above a place is its scheme.
    let page = |p: &mut Presenter<()>, view: u32| {
        ask(p, json!({"view": view, "names": ["page"]}))["colors"]["page"].as_i64()
    };
    assert_eq!(page(&mut p, outer), Some(argb(0xffffffff)));
    assert_eq!(page(&mut p, night), Some(argb(0xff000000)));

    // A name the plan lacks is null; the others still answer.
    let both = ask(
        &mut p,
        json!({"view": inner, "names": ["text.primary", "ink"]}),
    );
    assert_eq!(both["colors"]["text.primary"], Value::Null);
    assert_eq!(both["colors"]["ink"].as_i64(), Some(bold));
    assert_eq!(both["inputs"].as_str(), Some(inner_inputs.as_str()));

    // A view that is not live, and names that are not strings, are refused.
    assert!(ask(&mut p, json!({"view": 999_999, "names": ["ink"]}))["error"].is_string());
    assert!(ask(&mut p, json!({"view": 0, "names": [1]}))["error"].is_string());
    assert!(ask(&mut p, json!({"view": 0}))["error"].is_string());

    // A live batch moves the colour and the inputs.
    let batch =
        design::batch(&design::shape(&a), &design::tokens(&a), &design::tokens(&b)).unwrap();
    assert_eq!(p.apply_live(&batch.json()), None);
    let (green, after_batch) = ink(&mut p, inner);
    assert_eq!(green, argb(0xff00ff00));
    assert_ne!(after_batch, inner_inputs, "a batch moves the inputs");
    assert_eq!(
        ink(&mut p, outer).0,
        plain,
        "a place it did not reach keeps its colour"
    );

    // `setMode` moves the window's tuple: the inputs, and what the root answers.
    let (_, before_mode) = ink(&mut p, outer);
    assert_ne!(
        before_mode, outer_inputs,
        "the batch moved the outer inputs too"
    );
    assert_eq!(p.set_modes(&[("tone", "bold")]), None);
    let (bold_now, after_mode) = ink(&mut p, outer);
    assert_ne!(after_mode, before_mode, "setMode moves the inputs");
    assert_eq!(bold_now, argb(0xff00ff00));
    assert_eq!(ink(&mut p, 0).0, argb(0xff00ff00));
    // Nothing moved: the inputs hold.
    assert_eq!(ink(&mut p, outer).1, after_mode);
}

/// Stage 6 G: Android's `length` and `number` (and the fonts a native view
/// may read) ask by the same entry, a place's own tuple under a pin.
#[test]
fn a_view_asks_its_lengths_and_numbers_by_name() {
    let dir = std::env::temp_dir().join(format!("exact-linux-tamber-kinds-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\naxes\n  tone = plain | bold\n\ntheme\n  space.row = 12px\n    when tone=bold: 8px\n  opacity.faded: number = 0.5\n  type.sys = 600 1rem system-ui\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column width=200 height=100 padding=\"space.row\" opacity=\"opacity.faded\"\n      text \"a\" testId=\"outer\" font=\"type.sys\"\n      column -exact-mode=\"tone=bold\" padding=\"space.row\"\n        text \"b\" testId=\"inner\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let (mut p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        (200., 100.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let (outer, inner) = (id(&p, "outer"), id(&p, "inner"));
    let request = |view| json!({"view": view, "names": [], "lengths": ["space.row", "no.such"], "numbers": ["opacity.faded"], "fonts": ["type.sys"]});
    let a = ask(&mut p, request(outer));
    assert_eq!(a["lengths"]["space.row"], 12.0, "{a}");
    assert_eq!(a["lengths"]["no.such"], Value::Null);
    assert_eq!(a["numbers"]["opacity.faded"], 0.5);
    assert_eq!(a["fonts"]["type.sys"]["weight"], 600);
    assert_eq!(a["fonts"]["type.sys"]["size"], 16.0);
    let b = ask(&mut p, request(inner));
    assert_eq!(b["lengths"]["space.row"], 8.0, "a pin's tuple: {b}");
}
