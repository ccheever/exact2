//! The driver's requests, one JSON line each on stdin. While the loop waits
//! for the next one, a reply the executor queues commits when it lands, as
//! on the other hosts: the app goes on with what the reply starts (a renewal
//! after a 401) while the driver talks to a server, and the clock still
//! moves only by `clock`.

use crate::presenter::Presenter;
use exact_runner::DataSource;

/// Hand each request line to `serve` until stdin closes or `serve` says to
/// stop, committing replies and pictures that land in between.
#[cfg(unix)]
pub(super) fn each_request<D: DataSource + Default>(
    p: &mut Presenter<D>,
    mut serve: impl FnMut(&mut Presenter<D>, &str) -> bool,
) {
    use std::io::Read;
    let mut pending = Vec::new();
    let mut chunk = vec![0; 64 * 1024];
    loop {
        while let Some(end) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let Ok(line) = String::from_utf8(line) else {
                return;
            };
            if !serve(p, line.trim_end_matches(['\n', '\r'])) {
                return;
            }
        }
        let Some([input, replies, pictures]) = wait([0, p.executor_fd(), p.image_fd()]) else {
            return;
        };
        if replies || pictures {
            arrived(p);
        }
        if input {
            // One read of what poll said is there: the reader holds nothing
            // back, so a line never waits behind a buffer poll cannot see.
            match std::io::stdin().lock().read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => pending.extend_from_slice(&chunk[..n]),
            }
        }
    }
}

/// Windows waits on stdin alone: its executor has no descriptor to poll,
/// so a reply commits at the next request.
#[cfg(not(unix))]
pub(super) fn each_request<D: DataSource + Default>(
    p: &mut Presenter<D>,
    mut serve: impl FnMut(&mut Presenter<D>, &str) -> bool,
) {
    use std::io::BufRead;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { return };
        if !serve(p, &line) {
            return;
        }
    }
}

/// What a wake brings between requests: replies into the runner, the
/// commands their commits asked for, and loaded pictures' sizes. The
/// picture itself is painted at the next request, which is presentation in
/// the headless carrier.
#[cfg(unix)]
fn arrived<D: DataSource + Default>(p: &mut Presenter<D>) {
    if let Some(e) = p.pump(p.host().now()) {
        eprintln!("exact: {e}");
    }
    p.poll_update();
    p.poll_development(D::default);
    p.run_commands(D::default);
    p.poll_images();
}

/// Which of stdin, the executor and the image workers is readable; `None`
/// when polling fails. A signal that interrupts the poll is a turn with none.
#[cfg(unix)]
#[allow(unsafe_code)] // poll through libc
fn wait(fds: [i32; 3]) -> Option<[bool; 3]> {
    let mut pfds = fds.map(|fd| libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    });
    // SAFETY: the array is ours and sized by its length; poll reads and
    // writes only inside it.
    let result = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, -1) };
    if result < 0 {
        return (std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted)
            .then_some([false; 3]);
    }
    // A closed stdin is readable: its read answers 0 and ends the loop.
    let ready = libc::POLLIN | libc::POLLHUP | libc::POLLERR;
    Some(pfds.map(|fd| fd.revents & ready != 0))
}
