//! LLP 1069.005 D1b through the Hermes executor: ECDSA P-256 with the web's
//! names and errors, checked against `exact_data::crypto` (RustCrypto's
//! P-256) as the reference, both ways. Signatures are randomized, so a test
//! verifies them, never compares them. `js/web/tests/browser.rs` holds the
//! browser realms (Chrome's WebCrypto) to the same checks.
#![cfg(exact_js_engine)]

use exact_data::crypto::{generate_p256, unbase64url, EcKey, Jwk};
use exact_js::Module;
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ecdsa.hbc"));
const GRANTS: &str = "secret.keep dpop\n";

const SRC: &str = r#"
component App
  mutation result as shape string
  action initKey writes result
    send result = initKey()
  action keypair writes result
    send result = keypair()
  action importSign writes result
    send result = importSign("")
  action roundTrip writes result
    send result = roundTrip("")
  action refusals writes result
    send result = refusals()
  action keep writes result
    send result = keep()
  action kept writes result
    send result = kept()
  view
    column
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("ecdsa fixture compiles")
}

fn module() -> Module {
    let mut module = Module::loaded(HBC.to_vec(), "test.ecdsa", GRANTS).expect("loads");
    module.set_budget_ms(f64::INFINITY);
    module.bind(&plan());
    module
}

fn text(answer: Answer) -> String {
    match answer {
        Answer::Now(value) => value.as_str().expect("a string").to_string(),
        Answer::Later(_) => panic!("expected an answer now"),
    }
}

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn jwk(v: &serde_json::Value) -> Jwk {
    Jwk::from_json(&v.to_string()).expect("a P-256 JWK")
}

#[test]
fn a_generated_key_exports_what_rust_imports_and_its_signature_verifies() {
    let mut module = module();
    let mut store = Store::new(GRANTS, vec![]);
    let reply: serde_json::Value =
        serde_json::from_str(&text(module.answer(&mut store, "keypair", &[]).unwrap())).unwrap();
    assert_eq!(
        reply["shape"],
        "private true sign public verify P-256 [object CryptoKey] true"
    );
    assert_eq!(reply["priv"]["kty"], "EC");
    assert_eq!(reply["priv"]["ext"], true);
    assert_eq!(reply["priv"]["key_ops"], serde_json::json!(["sign"]));
    // Rust imports both JWKs; the private one's point is the public one's.
    let public = EcKey::from_jwk(&jwk(&reply["pub"]), true).unwrap();
    let private = EcKey::from_jwk(&jwk(&reply["priv"]), true).unwrap();
    assert_eq!(
        private.public_key().to_jwk().unwrap(),
        public.to_jwk().unwrap()
    );
    let signature = unhex(reply["signature"].as_str().unwrap());
    assert_eq!(signature.len(), 64, "raw r‖s");
    assert!(public.verify(b"proof", &signature));
    assert!(!public.verify(b"other", &signature));
    // Generating and signing are reads; exporting is not.
    assert_eq!(store.entropy_draws(), 2);
}

#[test]
fn a_rust_key_imports_here_and_signs_what_rust_verifies_and_import_is_pure() {
    let mut module = module();
    let mut store = Store::new(GRANTS, vec![]);
    let pair = generate_p256(&Store::new("", []), true).unwrap();
    let private = pair.private.to_jwk().unwrap().to_json();
    let signature = text(
        module
            .answer(&mut store, "importSign", &[Value::str(&private)])
            .unwrap(),
    );
    assert!(pair.public.verify(b"imported", &unhex(&signature)));
    assert_eq!(store.entropy_draws(), 1, "the sign, not the import");
    let public = pair.public.to_jwk().unwrap();
    let back = text(
        module
            .answer(&mut store, "roundTrip", &[Value::str(&public.to_json())])
            .unwrap(),
    );
    assert_eq!(back, format!("EC P-256 {} {} true", public.x, public.y));
    assert_eq!(store.entropy_draws(), 1, "import and export are pure");
    assert!(unbase64url(&public.x).is_some_and(|x| x.len() == 32));
}

#[test]
fn refusals_are_the_webs_and_a_key_at_initialization_is_refused() {
    let mut module = module();
    let mut store = Store::new(GRANTS, vec![]);
    assert!(text(module.answer(&mut store, "initKey", &[]).unwrap())
        .contains("unavailable during module initialization; call it inside an answer"));
    assert_eq!(
        text(module.answer(&mut store, "refusals", &[]).unwrap()),
        "NotSupportedError/NotSupportedError/SyntaxError/InvalidAccessError/NotSupportedError/NotSupportedError/InvalidAccessError/NotSupportedError/NotSupportedError"
    );
}

#[test]
fn a_kept_key_is_a_jwk_under_the_secret_and_signs_after_a_reload() {
    let mut module = module();
    let mut store = Store::new(GRANTS, vec![]);
    let reply: serde_json::Value =
        serde_json::from_str(&text(module.answer(&mut store, "keep", &[]).unwrap())).unwrap();
    assert_eq!(
        reply["extractable"], false,
        "a kept key comes back non-extractable"
    );
    let public = EcKey::from_jwk(&jwk(&reply["pub"]), true).unwrap();
    assert!(public.verify(b"kept", &unhex(reply["signature"].as_str().unwrap())));
    // Rust wrote the JWK (with `d`) under the secret itself.
    let kept = Jwk::from_json(store.get("dpop").unwrap()).unwrap();
    assert!(kept.d.is_some() && kept.x == public.to_jwk().unwrap().x);
    // A new executor over the host's kept secrets: the same key signs.
    let mut later = module_again();
    let mut reloaded = Store::new(GRANTS, store.snapshot());
    let signature = text(later.answer(&mut reloaded, "kept", &[]).unwrap());
    assert!(public.verify(b"kept", &unhex(&signature)));
    let mut empty = Store::new(GRANTS, vec![]);
    assert_eq!(text(later.answer(&mut empty, "kept", &[]).unwrap()), "none");
}

fn module_again() -> Module {
    module()
}
