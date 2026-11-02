use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

const BOUND: Duration = Duration::from_secs(10);

struct ChildServer {
    child: Child,
    dist: PathBuf,
}

impl Drop for ChildServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dist);
    }
}

#[test]
fn child() {
    let Some(dist) = std::env::var_os("EXACT_RENDER_STDOUT_TEST") else {
        return;
    };
    // Match the shipping server's panic=abort, including panics on workers.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        hook(info);
        std::process::abort();
    }));
    let plan = contract::compile(
        "routes nav\n  tab home \"/\" render=request\n\ncomponent App\n  view\n    text \"Still serving\"\n",
    )
    .unwrap();
    let server = Server::bind(
        Serve {
            dist: dist.into(),
            port: 0,
            name: "Closed stdout".into(),
            origin: None,
            deadline: BOUND,
            renders: 1,
            queue: 1,
            viewport: Default::default(),
            lifetime: Duration::from_secs(60),
            generations: None,
        },
        plan,
        "",
    )
    .unwrap();
    struct Nothing;
    impl DataSource for Nothing {
        fn query(
            &mut self,
            source: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(source.into()))
        }
    }
    let result = run_server(server, || Nothing);
    // The test harness would write its result to the closed stdout pipe.
    std::process::exit(if result == std::process::ExitCode::SUCCESS {
        0
    } else {
        1
    });
}

fn closed_stdout(requests: usize) {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dist = std::env::temp_dir().join(format!(
        "exact-render-stdout-{}-{requests}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&dist).unwrap();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/index.html"),
        dist.join("shell.html"),
    )
    .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "stdout_tests::child", "--nocapture"])
        .env("EXACT_RENDER_STDOUT_TEST", &dist)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut server = ChildServer { child, dist };
    let stdout = server.child.stdout.take().unwrap();
    let (ready, when_ready) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        while stdout.read_line(&mut line).unwrap() != 0 {
            // A serial test harness prefixes its status on this same line.
            if let Some((_, ready_line)) = line.split_once("serving http://") {
                let (addr, _) = ready_line.split_once('/').unwrap();
                let addr: SocketAddr = addr.parse().unwrap();
                ready.send((addr, stdout)).unwrap();
                return;
            }
            line.clear();
        }
    });
    let (addr, stdout) = when_ready.recv_timeout(BOUND).expect("server readiness");
    reader.join().unwrap();
    // An answered request proves the signal handler is installed: readiness
    // is printed just before its installation.
    assert!(request(addr).unwrap().starts_with("HTTP/1.1 200"));
    drop(stdout);

    let responses: Vec<_> = (0..requests).map(|_| request(addr)).collect();
    // SAFETY: this is the child we own, and it has not been reaped.
    assert_eq!(
        unsafe { libc::kill(server.child.id() as i32, libc::SIGTERM) },
        0
    );
    let until = Instant::now() + BOUND;
    let status = loop {
        if let Some(status) = server.child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < until, "server did not drain");
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    server
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(status.success(), "server exited {status}: {stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    for response in responses {
        let response = response.unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(|n| n.parse().unwrap())
            })
            .unwrap();
        assert_eq!(body.len(), length, "incomplete response");
        assert!(body.contains(">Still serving<"), "{body}");
    }
}

fn request(addr: SocketAddr) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(&addr, BOUND)?;
    stream.set_read_timeout(Some(BOUND))?;
    stream.set_write_timeout(Some(BOUND))?;
    stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[test]
fn requests_survive_closed_stdout() {
    closed_stdout(2);
}

#[test]
fn sigterm_drains_with_closed_stdout() {
    closed_stdout(0);
}
