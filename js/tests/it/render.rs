//! A page whose TypeScript source spins renders 503 at its deadline, and the
//! server keeps serving (LLP 1048.000 D10): the render's watchdog interrupts
//! the running call, which is refused, and the page shows its placeholder.

#![cfg(all(exact_js_engine, unix))]

use exact_js::Module;
use exact_plan::{Plan, Value};
use exact_render::{Serve, Server};
use exact_runner::{Answer, DataError, DataSource, Interrupt, Outcome, Store, Target};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/spin.hbc"));

/// A render entry's data source: a fresh module per render behind a
/// forwarder, the shape an app's own source has (Interview's, say).
struct Spin(Module);

impl Default for Spin {
    fn default() -> Spin {
        let mut module = Module::new(HBC.to_vec(), "test.spin", "");
        module.set_budget_ms(f64::INFINITY);
        Spin(module)
    }
}

impl DataSource for Spin {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.0.query(source, args)
    }
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.0.answer_for(target, store, source, args)
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.0.parse_for(target, store, source, args, outcome)
    }
    fn interrupt(&self) -> Option<Interrupt> {
        self.0.interrupt()
    }
    fn bind(&mut self, plan: &Plan) {
        self.0.bind(plan);
    }
    fn app_id(&self) -> &str {
        self.0.app_id()
    }
    fn grants(&self) -> &str {
        self.0.grants()
    }
    fn revision(&self) -> Option<&str> {
        self.0.revision()
    }
    fn ready(&self) -> bool {
        self.0.ready()
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.0.activate()
    }
}

fn start(deadline: Duration) -> SocketAddr {
    let dist = std::env::temp_dir().join(format!("exact-js-render-spin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dist);
    std::fs::create_dir_all(&dist).unwrap();
    let shell = Path::new(env!("CARGO_MANIFEST_DIR")).join("../host/web/index.html");
    std::fs::copy(shell, dist.join("shell.html")).unwrap();
    let serve = Serve {
        dist,
        port: 0,
        name: "Spin".into(),
        origin: None,
        deadline,
        // One worker: the second page renders only if the first let it go.
        renders: 1,
        queue: 4,
        viewport: Default::default(),
        lifetime: Duration::from_secs(120),
        generations: None,
    };
    let plan = contract::compile(super::interrupt::SRC).unwrap();
    let server = Server::bind(serve, plan, "").unwrap();
    let addr = server.addr();
    std::thread::spawn(move || server.run(Spin::default));
    addr
}

/// Status, headers (lowercased names) and body; a stuck render is a failure
/// here, not a hang.
fn get(addr: SocketAddr, path: &str) -> (u16, Vec<(String, String)>, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: spin.test\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("{path}: no answer: {e}"));
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_lowercase(), value.trim().to_string()))
        .collect();
    (status, headers, body.to_string())
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn a_spinning_source_renders_503_at_its_deadline_and_the_server_keeps_serving() {
    let deadline = Duration::from_millis(300);
    let addr = start(deadline);
    let started = Instant::now();
    let (status, headers, body) = get(addr, "/page/spin");
    let took = started.elapsed();
    assert_eq!(status, 503, "{body}");
    assert!(
        took >= deadline,
        "answered in {took:?}, before the deadline"
    );
    assert_eq!(header(&headers, "retry-after"), Some("1"));
    assert_eq!(header(&headers, "cache-control"), Some("no-store"));
    // The one worker is free: the next page renders, and says its words.
    let (status, _, body) = get(addr, "/page/calm");
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("calm"), "{body}");
    let (status, _, body) = get(addr, "/.exact/health");
    assert_eq!((status, body.as_str()), (200, "ok\n"));
}
