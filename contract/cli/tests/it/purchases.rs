//! Purchases' and store review's admission in the runner (LLP 1133.015),
//! through this checkout's `@exact/purchases` and `@exact/store-review`
//! installed beside the app, each implementation played by the test as the
//! host's native slot under the agent: the purchase sheet a request holds
//! and what its tap tells the implementation, the restore sheet Apple alone
//! shows, the origin rule (§10 drive 10), the catalogue the implementation is
//! handed, a stale time that asks the source again (D7), and the targets
//! with no store.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use listPurchases, purchase, restorePurchases, purchasesStatus, PurchaseDetails, PurchasesStatus from "@exact/purchases"
use requestReview from "@exact/store-review"

component App
  resource status = purchasesStatus() as shape PurchasesStatus
  resource owned = listPurchases() as shape PurchaseDetails[]
  mutation bought as shape PurchaseDetails
  mutation restored as shape PurchaseDetails[]
  action buy
    send bought = purchase(itemId="plus.yearly")
  action restore
    send restored = restorePurchases()
  action review
    requestReview()
  view
    column
      text `${owned.length}`
"#;

const CATALOG: &str = r#"[{"itemId": "plus.yearly", "title": "Yearly", "type": "subscription", "price": {"currency": "USD", "value": "19.99"}}]"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-purchases-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["purchases", "store-review"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
        }
        std::fs::write(path.join("app.contract"), APP).unwrap();
        std::fs::write(path.join("catalog.json"), CATALOG).unwrap();
        let manifest = json!({ "name": "Purchases", "app": { "id": "com.example.purchases", "name": "Purchases" }, "purchases": { "catalog": "catalog.json" } });
        std::fs::write(path.join("app.json"), manifest.to_string()).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Grants;
impl exact_runner::DataSource for Grants {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        ""
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

/// The subscription the implementation lists, and when it ends.
const EXPIRES: f64 = 1_790_000_060_000.0;

/// A runner over the app on `target`, `bound` implementations bound under
/// the agent and played by the native slot: `listPurchases` answers one
/// subscription that goes stale at [`EXPIRES`].
fn boot(name: &str, target: &str, bound: &[&str]) -> (App, Runner<Grants>, Calls) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Grants,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target(target);
    r.set_time(1_790_000_000_000.0, 0.0).unwrap();
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v.clone());
            let row = json!({ "itemId": "plus.yearly", "purchaseToken": "1", "purchaseState": "purchased", "purchaseTime": 0, "acknowledged": true, "willAutoRenew": true, "quantity": 1, "expirationTime": EXPIRES, "apple": null, "android": null });
            let answer = match (v["send"].as_str(), v["source"].as_str()) {
                (Some("purchase"), _) => row,
                (Some("restorePurchases"), _) => json!({ "exact:answer": [row] }),
                (_, Some("listPurchases")) => json!({ "exact:answer": [row], "exact:stale": EXPIRES }),
                (_, Some("purchasesStatus")) => json!({ "available": true, "store": "app-store", "canMakePayments": true, "storefront": "US", "apple": null, "android": null }),
                _ => json!({}),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    r.bind_extensions(&[], bound);
    (app, r, calls)
}

fn pump(r: &mut Runner<Grants>, parked: &mut Vec<(u64, u64)>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        if outs.is_empty() && released.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let dispatch = match out.request.continuation {
                Some(token) => {
                    parked.push((token, out.ticket));
                    r.dispatch_work(token)
                }
                None => r.native_work(&out.request),
            };
            run(r, out.ticket, dispatch);
        }
        for (token, dispatch) in released {
            let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
            run(r, ticket, dispatch);
        }
    }
}

fn run(r: &mut Runner<Grants>, ticket: u64, dispatch: Dispatch) {
    let outcome = match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.recv().unwrap()
        }
        _ => return,
    };
    let _ = r.fulfill(ticket, outcome);
}

fn called(calls: &Calls, key: &str, name: &str) -> Vec<Json> {
    (calls.lock().unwrap().iter())
        .filter(|c| c[key] == name)
        .cloned()
        .collect()
}

fn logs(r: &Runner<Grants>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn tap(r: &mut Runner<Grants>, parked: &mut Vec<(u64, u64)>, ticket: u64, choice: &str) {
    let line = json!({ "op": "tap", "ticket": ticket, "choice": choice }).to_string();
    let (said, _) = exact_runner::agent::answer(r, &line).expect("a module's ticket");
    assert!(!said.contains("\"error\""), "{said}");
    pump(r, parked);
}

/// A purchase holds a `purchase` sheet whose tap reaches the implementation
/// as the call's `preferred`; the implementation is handed the app's
/// catalogue; a tool's purchase and a tool's review request fail
/// `person-required` before any sheet (§10 drive 10).
#[test]
fn a_purchase_holds_its_sheet_and_a_tool_cannot_buy() {
    let (_app, mut r, calls) = boot("buy", "ios", &["purchases", "store-review"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let told = called(&calls, "extension", "purchases");
    assert!(
        told.iter()
            .any(|c| c["preferred"]["catalog"][0]["itemId"] == "plus.yearly"),
        "the catalogue reaches the implementation: {told:?}"
    );
    r.act("buy", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{}", logs(&r));
    assert_eq!(holds[0].capability, "purchase");
    assert_eq!(holds[0].choices, ["buy", "pending", "unavailable", "fail"]);
    assert!(called(&calls, "send", "purchase").is_empty(), "held first");
    let ticket = holds[0].ticket;
    tap(&mut r, &mut parked, ticket, "buy");
    let asked = called(&calls, "send", "purchase");
    assert_eq!(asked.len(), 1, "{}", logs(&r));
    assert_eq!(asked[0]["preferred"], json!({ "tap": "buy" }));

    r.act_from(CallOrigin::Tool, "buy", Vec::new()).unwrap();
    r.act_from(CallOrigin::Tool, "review", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
    assert_eq!(called(&calls, "send", "purchase").len(), 1);
    assert!(called(&calls, "send", "requestReview").is_empty());
    let said = logs(&r);
    assert!(
        said.contains("purchases.purchase: refused, person-required (tool origin)"),
        "{said}"
    );
    assert!(
        said.contains("store-review.requestReview: refused, person-required (tool origin)"),
        "{said}"
    );
}

/// Restoring asks on Apple's targets alone: a `restore` sheet there, and on
/// Android the implementation's query with no sheet.
#[test]
fn restoring_asks_on_apple_alone() {
    let (_app, mut r, calls) = boot("restore", "macos", &["purchases"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("restore", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{}", logs(&r));
    assert_eq!(
        (holds[0].capability.as_str(), holds[0].choices.clone()),
        ("restore", vec!["allow".to_owned()])
    );

    let (_app, mut r, calls_android) = boot("restore-android", "android", &["purchases"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("restore", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
    assert_eq!(called(&calls_android, "send", "restorePurchases").len(), 1);
    drop(calls);
}

/// `listPurchases`' answer names when it goes stale: the runner takes the
/// time off the answer, and asks the source again when the session's wall
/// clock reaches it, once (D7).
#[test]
fn a_stale_time_asks_again() {
    let (_app, mut r, calls) = boot("stale", "ios", &["purchases"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let reads = || called(&calls, "source", "listPurchases").len();
    let first = reads();
    assert!(first >= 1, "{}", logs(&r));
    assert!(
        format!("{:?}", r.resource("owned")).contains("plus.yearly"),
        "the list, unwrapped: {:?}",
        r.resource("owned")
    );
    r.advance(59_000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(reads(), first, "not before its time: {}", logs(&r));
    r.advance(61_000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(reads(), first + 1, "{}", logs(&r));
    assert!(
        logs(&r).contains("purchases.listPurchases: stale, asked again"),
        "{}",
        logs(&r)
    );
}

/// A target with no store: the status reads its default, the list is
/// empty, and a purchase fails `NotSupportedError` before any sheet (D20).
#[test]
fn the_web_has_no_store() {
    let (_app, mut r, calls) = boot("web", "web", &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("buy", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
    assert!(calls.lock().unwrap().is_empty());
    let status = format!("{:?}", r.resource("status"));
    assert!(
        status.contains("Bool(false)") && status.contains(r#"Str("none")"#),
        "{status}"
    );
    assert!(!format!("{:?}", r.resource("bought")).contains("plus.yearly"));
    assert!(logs(&r).contains("NotSupportedError"), "{}", logs(&r));
}
