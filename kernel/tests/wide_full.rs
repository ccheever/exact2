//! A full table whose free-able slots are all recent recovers as refused
//! colours age them, with no new colour admitted (LLP 1121 stage 4). Its own
//! process: the table is the process's.

use exact_kernel::style::wide::{self, Keep};
use exact_kernel::ColorValue;

fn srgb(i: usize) -> String {
    format!(
        "color(srgb {} {} 0.25)",
        (i % 64) as f64 / 64.0,
        (i / 64) as f64 / 64.0
    )
}

#[test]
fn refusals_age_a_full_table_until_it_admits_again() {
    // Three quarters and one held, as by a kernel that holds them.
    let held: Vec<ColorValue> = (0..3_073)
        .map(|i| ColorValue::parse_light_dark(&srgb(i)).unwrap())
        .collect();
    let _keep = Keep::of(|m| held.iter().for_each(|c| m.color(*c)));
    // The rest filled with colours nothing holds, each too recent to free.
    for i in 3_073..4_096 {
        assert!(ColorValue::parse_light_dark(&srgb(i)).is_some());
    }
    assert_eq!(wide::used(), 4_096);
    let refusals = wide::refusals();
    let fresh = "color(display-p3 0.123 0.456 0.789)";
    let mut tries = 0;
    while ColorValue::parse_light_dark(fresh).is_none() {
        tries += 1;
        assert!(tries <= wide::GRACE as usize, "never admitted");
    }
    assert!(tries > 0, "refused at first");
    assert_eq!(wide::refusals(), refusals + tries as u64);
}

#[cfg(unix)]
#[test]
fn full_table_refusal_survives_blocked_stderr() {
    use std::io::{ErrorKind, Write};
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Command;

    let (mut writer, _reader) = UnixStream::pair().unwrap();
    writer.set_nonblocking(true).unwrap();
    loop {
        match writer.write(b"x") {
            Ok(n) => assert!(n > 0),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("fill stderr: {e}"),
        }
    }
    assert_eq!(
        writer.write(b"x").unwrap_err().kind(),
        ErrorKind::WouldBlock
    );

    // Isolate stderr from this harness; capture would hide eprintln!'s failure.
    // Keep the reader open and undrained until the child exits.
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "refusals_age_a_full_table_until_it_admits_again",
            "--nocapture",
        ])
        .stderr(OwnedFd::from(writer))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{}: {stdout}", output.status);
    assert!(stdout.contains("1 passed"), "{stdout}");
}
