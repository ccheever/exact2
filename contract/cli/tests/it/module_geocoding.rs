//! LLP 1133.021 §12 for `@exact/geocoding`, the package copied out of the
//! checkout as an installed package is: each send answers its preferred
//! value, an error preferred fails it with that code, and with nothing
//! preferred it answers its declared default, `[]`; on the web, which has
//! no geocoder, the status reads `available` false and each send fails.

use exact_runner::{agent, Runner, Value};
use std::path::{Path, PathBuf};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-geocode-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/geocoding");
        copy(&package, &path.join("node_modules/@exact/geocoding"));
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn plan(&self) -> exact_plan::Plan {
        contract::compile_path_all(&self.0.join("app.contract"), false)
            .map(|(plan, _)| plan)
            .unwrap_or_else(|e| panic!("{e:?}"))
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

const APP: &str = r#"use geocode, reverseGeocode, geocodingStatus, Place, GeocodingStatus from "@exact/geocoding"

component App
  resource status = geocodingStatus() as shape GeocodingStatus
  mutation places as shape Place[]
  action named
    send places = reverseGeocode(latitude=48.8566, longitude=2.3522)
  action found
    send places = geocode(address="1 Infinite Loop")
  view
    column
      text testId="available" `${status.available}`
      match places
        case some(p)
          text testId="count" `${p.length}`
        case none
          text testId="count" "none"
"#;

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}

fn boot(app: &App, target: &str) -> Runner<NoData> {
    let mut r = Runner::boot(
        app.plan(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target(target);
    r
}

fn logs(r: &Runner<NoData>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn slot(r: &Runner<NoData>, name: &str) -> String {
    format!("{:?}", r.resource(name).or_else(|| r.slot(name)))
}

fn pump(r: &mut Runner<NoData>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let (asked, error) = r.apply_announced();
        assert!(error.is_none(), "{error:?}");
        if outs.is_empty() && asked.is_empty() {
            return;
        }
        for out in outs {
            let token = out.request.continuation.expect("held work");
            if let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) =
                r.dispatch_work(token)
            {
                r.fulfill(out.ticket, work()).unwrap();
            }
        }
    }
}

#[test]
fn a_geocode_answers_what_the_drive_preferred_or_nothing() {
    let app = App::new("ios");
    let mut r = boot(&app, "ios");
    pump(&mut r);
    assert!(
        slot(&r, "status").contains("true"),
        "{}",
        slot(&r, "status")
    );
    r.act("found", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        slot(&r, "places").contains("[]") || slot(&r, "places").contains("List([])"),
        "{} {}",
        slot(&r, "places"),
        logs(&r)
    );

    let paris = r#"{"op":"prefer","extension":{"geocoding.reverseGeocode":[{"latitude":48.8566,"longitude":2.3522,"city":"Paris","countryCode":"FR","country":"France"}]}}"#;
    r.extension_agent(paris).unwrap().1.unwrap();
    r.act("named", Vec::new()).unwrap();
    pump(&mut r);
    let places = slot(&r, "places");
    assert!(
        places.contains("Paris") && places.contains("FR"),
        "{places} {}",
        logs(&r)
    );

    let failing =
        r#"{"op":"prefer","extension":{"geocoding.reverseGeocode":{"error":"NetworkError"}}}"#;
    r.extension_agent(failing).unwrap().1.unwrap();
    r.act("named", Vec::new()).unwrap();
    pump(&mut r);
    assert!(logs(&r).contains("NetworkError"), "{}", logs(&r));
}

#[test]
fn the_web_has_no_geocoder() {
    let app = App::new("web");
    let mut r = boot(&app, "web");
    pump(&mut r);
    assert!(
        slot(&r, "status").contains("false"),
        "{}",
        slot(&r, "status")
    );
    r.act("found", Vec::new()).unwrap();
    pump(&mut r);
    assert!(logs(&r).contains("NotSupportedError"), "{}", logs(&r));
}
