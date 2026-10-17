//! @ref LLP 1121 D15 (stage 7 D) — a component that isn't Contract declares
//! its params in the design (`component MapCard` over `params`): named by the
//! design file and the component, checked as a Contract param is, kept by its
//! declaration though no row reads it, and listed beside Contract ones.

use exact_plan::Plan;
use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "exact2-native-params-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

fn theme(plan: &Plan) -> Vec<(String, String)> {
    plan.theme
        .iter()
        .map(|t| (plan.str(t.name).to_string(), plan.str(t.value).to_string()))
        .collect()
}

const VIEW: &str = "component App\n  view\n    text \"a\" color=\"ink\"\n";

const MAP_CARD: &str = "tamber t\n\naxes\n  tone = warm | cool\n\ntheme\n  ink = #111111\n  accent.fill = #c8102e\n    when tone=cool: #0a7aff\n\nexports\n  ink\n\ncomponent MapCard\n  params\n    pin: color = accent.fill  control=color  priority=1\n    inset: length = 8px  in 0..24  step=2  control=slider  group=\"Shape\"\n    halo: color = color-mix(in oklab, param.pin 40%, white)  control=none\n";

#[test]
fn a_native_components_params_are_kept_named_and_listed() {
    let path = app("kept", &[("app.contract", VIEW), ("app.tamber", MAP_CARD)]);
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let rows = theme(&plan);
    let token = |param: &str| contract_lower::params::token_name("app.tamber#MapCard", param);
    // No row reads them and `exports` lists none: their declaration keeps them.
    for param in ["pin", "inset", "halo"] {
        assert!(
            rows.iter().any(|(n, _)| *n == token(param)),
            "{param}: {rows:?}"
        );
    }
    assert!(
        rows.iter().any(|(n, v)| *n == token("inset") && v == "8px"),
        "{rows:?}"
    );
    // And what a default reads, so an edit to it reaches the param live.
    assert!(rows.iter().any(|(n, _)| n == "accent.fill"), "{rows:?}");
    // Listed beside Contract components, by identity; a default names a
    // param by its full name.
    let params = contract::tamber_params::params_of(&path);
    let design = exact_tamber::parse(MAP_CARD).unwrap();
    let block = contract::tamber_params::components(
        &params,
        Some(&design),
        &contract_lower::tamber_vocab(),
    );
    let c = &block[0];
    assert_eq!(c["identity"], "app.tamber#MapCard");
    let param = |name: &str| {
        c["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["param"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(param("inset")["range"], serde_json::json!([0.0, 24.0]));
    assert_eq!(param("inset")["control"], "slider");
    assert_eq!(param("inset")["group"], "Shape");
    assert_eq!(param("pin")["short"], "MapCard.pin");
    assert_eq!(
        param("pin")["values"],
        serde_json::json!(["#c8102eff", "#0a7affff"])
    );
    assert_eq!(
        param("halo")["default"],
        "color-mix(in oklab, app.tamber#MapCard.pin 40%, white)"
    );
    // A full or short name resolves to it.
    assert_eq!(
        contract_lower::params::resolve(&params.tokens, &params.identities, "MapCard.inset")
            .unwrap()
            .token,
        token("inset")
    );
}

#[test]
fn a_native_components_params_are_refused_as_a_contract_ones() {
    let refused = |name: &str, params: &str| {
        let design = format!("tamber t\n\ntheme\n  ink = #111111\n  gap = 1rem\n\ncomponent MapCard\n  params\n{params}");
        let path = app(name, &[("app.contract", VIEW), ("app.tamber", &design)]);
        let e = contract::compile_path(&path).unwrap_err();
        (vec![e.id.to_string()], e.message)
    };
    let (ids, said) = refused("kind", "    pin: colour = red\n");
    assert_eq!(ids, ["tamber-param-kind"], "{said}");
    let (ids, said) = refused("rem", "    inset: length = gap  in 0..24\n");
    assert_eq!(ids, ["tamber-range-unit"], "{said}");
    let (ids, said) = refused("outside", "    inset: length = 30px  in 0..24\n");
    assert_eq!(ids, ["tamber-param-range"], "{said}");
    let (ids, said) = refused("attr", "    pin: color = red  colour=red\n");
    assert_eq!(ids, ["syntax-param-attribute"], "{said}");
    let (ids, said) = refused("range-meta", "    inset: length = 4px  range=\"0..24\"\n");
    assert_eq!(ids, ["syntax-param-attribute"], "{said}");
    let (ids, said) = refused("unknown", "    pin: color = param.nope\n");
    assert_eq!(ids, ["tamber-param-unknown"], "{said}");
    assert!(said.contains("its params: pin"), "{said}");
    let (ids, said) = refused("control", "    pin: color = red  control=slider\n");
    assert_eq!(ids, ["tamber-param-control"], "{said}");
}

#[test]
fn a_native_and_a_contract_component_of_one_name_share_no_short_name() {
    let card = "component MapCard\n  params\n    inset: length = \"4px\"\n  view\n    column padding=\"param.inset\"\n      text \"a\"\n";
    let root = "use MapCard from \"./card.contract\"\n\ncomponent App\n  view\n    MapCard()\n";
    let path = app(
        "shared",
        &[
            ("app.contract", root),
            ("card.contract", card),
            ("app.tamber", MAP_CARD),
        ],
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let params = contract::tamber_params::params_of(&path);
    let e = contract_lower::params::resolve(&params.tokens, &params.identities, "MapCard.inset")
        .unwrap_err();
    assert_eq!(e.id, "tamber-param-ambiguous");
    assert!(contract_lower::params::resolve(
        &params.tokens,
        &params.identities,
        "app.tamber#MapCard.inset"
    )
    .is_ok());
}

fn gen(target: &str, dir: &std::path::Path) -> (bool, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "gen", target])
        .arg(dir)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// The bindings name a param by `<Component>.<param>` (D12's mapping), its
/// value the token name the runtimes resolve; never the hash as a name.
#[test]
fn the_bindings_name_a_param_by_its_component() {
    let path = app("gen", &[("app.contract", VIEW), ("app.tamber", MAP_CARD)]);
    let dir = path.parent().unwrap();
    let pin = contract_lower::params::token_name("app.tamber#MapCard", "pin");
    let inset = contract_lower::params::token_name("app.tamber#MapCard", "inset");
    let read = |file: &str| std::fs::read_to_string(dir.join(".exact/tamber").join(file)).unwrap();
    for target in ["swift", "rust", "ts", "kotlin"] {
        let (ok, said) = gen(target, dir);
        assert!(ok, "{target}: {said}");
    }
    let swift = read("tamber.swift");
    assert!(
        swift.contains(&format!(
            "    static let mapCardPin = ExactTamberToken(\"{pin}\")"
        )),
        "{swift}"
    );
    assert!(
        swift.contains(&format!(
            "    static let mapCardInset = ExactTamberToken(\"{inset}\")"
        )),
        "a length too: {swift}"
    );
    assert!(swift.contains("/// `MapCard.inset`."), "{swift}");
    let rust = read("tamber.rs");
    assert!(
        rust.contains(&format!(
            "pub const MAP_CARD_INSET: Token<Length> = Token::new(\"{inset}\");"
        )),
        "{rust}"
    );
    let ts = read("tamber.ts");
    assert!(
        ts.contains(&format!("\"{pin}\"")) && ts.contains("  mapCardPin: { name: "),
        "{ts}"
    );
    let kotlin = read("TamberTokens.kt");
    assert!(kotlin.contains("package exact.app\n"), "{kotlin}");
    assert!(
        kotlin.contains("object TamberColor {\n")
            && kotlin.contains(&format!(
                "    /** `MapCard.pin`. */\n    const val MAP_CARD_PIN = \"{pin}\""
            )),
        "{kotlin}"
    );
    assert!(
        kotlin.contains(&format!("    const val MAP_CARD_INSET = \"{inset}\""))
            && kotlin.contains("object TamberLength {"),
        "{kotlin}"
    );
    assert!(
        !swift.contains(" param") && !rust.contains("PARAM_") && !kotlin.contains("PARAM_"),
        "no hash in a name"
    );
    // A token whose name maps as a param's does is refused, naming both.
    std::fs::write(
        dir.join("app.tamber"),
        MAP_CARD
            .replace("  ink\n", "  ink\n  map.card.pin\n")
            .replace(
                "  ink = #111111\n",
                "  ink = #111111\n  map.card.pin = #000000\n",
            ),
    )
    .unwrap();
    let (ok, said) = gen("swift", dir);
    assert!(
        !ok && said.contains("`MapCard.pin`") && said.contains("`map.card.pin`"),
        "{said}"
    );
}
