//! Completion of a cancellable, nonblocking Winsock connection.
//! @ref LLP 0068#windows-outbound-connection-readiness — peer address is not readiness

use crate::stdlib::abort::AbortSignal;
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::os::windows::io::AsRawSocket;
use std::time::{Duration, Instant};
use windows_sys::Win32::Networking::WinSock::{
    select, WSAGetLastError, FD_SET, SOCKET_ERROR, TIMEVAL,
};

pub(crate) fn connect_socket(
    address: SocketAddr,
    timeout: Duration,
    signal: &AbortSignal,
) -> io::Result<TcpStream> {
    check_abort(signal)?;
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(Protocol::TCP),
    )?;
    socket.set_nonblocking(true)?;
    complete(socket.connect(&address.into()), timeout, signal, || {
        ready(&socket)
    })?;
    socket.set_nonblocking(false)?;
    check_abort(signal)?;
    Ok(socket.into())
}

fn check_abort(signal: &AbortSignal) -> io::Result<()> {
    signal
        .check()
        .map_err(|error| io::Error::new(io::ErrorKind::Interrupted, error.to_string()))
}

fn complete(
    result: io::Result<()>,
    timeout: Duration,
    signal: &AbortSignal,
    mut poll: impl FnMut() -> io::Result<bool>,
) -> io::Result<()> {
    check_abort(signal)?;
    if let Err(error) = result {
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(error);
        }
        let deadline = Instant::now() + timeout;
        loop {
            check_abort(signal)?;
            if poll()? {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            // Stay on the owning caller's thread. Cancellation or failure drops
            // its one socket; there is no detached connection to clean up.
            std::thread::park_timeout(remaining.min(Duration::from_millis(10)));
        }
    }
    // Cancellation can race the readiness observation, including immediate
    // success. Never knowingly hand an aborted connection to the transport.
    check_abort(signal)
}

fn ready(socket: &Socket) -> io::Result<bool> {
    // select mutates each set. Rebuild both on every poll, including after a
    // previous no-readiness result emptied them. Winsock ignores nfds.
    let mut writable = FD_SET {
        fd_count: 1,
        ..FD_SET::default()
    };
    writable.fd_array[0] = socket.as_raw_socket() as _;
    let mut exceptional = writable;
    let timeout = TIMEVAL::default();
    // SAFETY: the sets and zero timeout live through this synchronous call;
    // socket remains owned and open, and each set contains exactly that socket.
    let result = unsafe {
        select(
            0,
            std::ptr::null_mut(),
            &mut writable,
            &mut exceptional,
            &timeout,
        )
    };
    if result == SOCKET_ERROR {
        // SAFETY: read the calling thread's error before making another OS call.
        return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
    }
    readiness(writable.fd_count != 0, exceptional.fd_count != 0, || {
        socket.take_error()
    })
}

fn readiness(
    writable: bool,
    exceptional: bool,
    take_error: impl FnOnce() -> io::Result<Option<io::Error>>,
) -> io::Result<bool> {
    if exceptional {
        return Err(take_error()?.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "connect exception without SO_ERROR",
            )
        }));
    }
    if !writable {
        // SO_ERROR can be empty while the connection is still pending, and
        // reading it here could consume an error before select reports it.
        return Ok(false);
    }
    match take_error()? {
        Some(error) => Err(error),
        None => Ok(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::abort::AbortController;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn pending() -> io::Result<()> {
        Err(io::ErrorKind::WouldBlock.into())
    }

    #[test]
    fn immediate_success_needs_no_readiness_poll() {
        complete(Ok(()), Duration::ZERO, &AbortSignal::default(), || {
            panic!("already connected")
        })
        .unwrap();
    }

    #[test]
    fn pending_connection_waits_until_ready() {
        let mut polls = 0;
        complete(
            pending(),
            Duration::from_secs(1),
            &AbortSignal::default(),
            || {
                polls += 1;
                Ok(polls == 3)
            },
        )
        .unwrap();
        assert_eq!(polls, 3);
    }

    #[test]
    fn pending_without_readiness_expires() {
        let error = complete(pending(), Duration::ZERO, &AbortSignal::default(), || {
            Ok(false)
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn abort_before_connect_skips_poll_even_for_immediate_success() {
        let controller = AbortController::new();
        controller.abort();
        for result in [Ok(()), pending()] {
            let error = complete(result, Duration::from_secs(1), &controller.signal(), || {
                panic!("aborted attempt polled")
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        }
    }

    #[test]
    fn cancellation_while_pending_or_during_readiness_prevents_handoff() {
        for ready in [false, true] {
            let controller = AbortController::new();
            let mut polls = 0;
            let error = complete(
                pending(),
                Duration::from_secs(1),
                &controller.signal(),
                || {
                    polls += 1;
                    controller.abort();
                    Ok(ready)
                },
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Interrupted);
            assert_eq!(polls, 1);
        }
    }

    #[test]
    fn connect_and_poll_failures_are_preserved() {
        let refused = complete(
            Err(io::ErrorKind::ConnectionRefused.into()),
            Duration::ZERO,
            &AbortSignal::default(),
            || panic!("failed attempt polled"),
        )
        .unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::ConnectionRefused);
        let failed = complete(pending(), Duration::ZERO, &AbortSignal::default(), || {
            Err(io::Error::from_raw_os_error(10038))
        })
        .unwrap_err();
        assert_eq!(failed.raw_os_error(), Some(10038));
    }

    #[test]
    fn only_writable_without_socket_error_is_success() {
        assert!(!readiness(false, false, || panic!("pending SO_ERROR consumed")).unwrap());
        assert!(readiness(true, false, || Ok(None)).unwrap());
        assert_eq!(
            readiness(true, false, || Ok(Some(
                io::ErrorKind::ConnectionRefused.into()
            )))
            .unwrap_err()
            .kind(),
            io::ErrorKind::ConnectionRefused
        );
        for writable in [false, true] {
            assert_eq!(
                readiness(writable, true, || Ok(None)).unwrap_err().kind(),
                io::ErrorKind::ConnectionAborted
            );
            assert_eq!(
                readiness(writable, true, || Ok(Some(
                    io::ErrorKind::ConnectionRefused.into()
                )))
                .unwrap_err()
                .kind(),
                io::ErrorKind::ConnectionRefused
            );
        }
    }

    #[test]
    fn local_winsock_connection_is_usable_at_handoff() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut stream = connect_socket(
            listener.local_addr().unwrap(),
            Duration::from_secs(1),
            &AbortSignal::default(),
        )
        .unwrap();
        // The original premature handoff failed at TCP_NODELAY or first write.
        stream.set_nodelay(true).unwrap();
        stream.write_all(b"connected").unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut bytes = [0; 9];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"connected");
    }

    #[test]
    fn preaborted_connect_opens_no_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let controller = AbortController::new();
        controller.abort();
        let error = connect_socket(
            listener.local_addr().unwrap(),
            Duration::from_secs(1),
            &controller.signal(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn local_winsock_refusal_is_not_a_connected_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error =
            connect_socket(address, Duration::from_secs(3), &AbortSignal::default()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionRefused);
    }
}
