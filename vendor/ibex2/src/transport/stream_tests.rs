//! Exercise both socket transports against real HTTP peers, not mock readers.
use super::{dev_tcp::DevTcpTransport, rustls_http::RustlsHttpTransport};
use crate::stdlib::abort::{AbortController, AbortSignal};
use crate::stdlib::fetch::{Request, Transport};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc};
use std::time::Duration;

fn transports() -> Vec<Arc<dyn Transport>> {
    vec![
        Arc::new(DevTcpTransport::new()),
        Arc::new(RustlsHttpTransport::new()),
    ]
}
fn request_head(socket: &mut TcpStream) {
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
        assert!(head.len() < 64 * 1024);
    }
}
fn server(work: impl FnOnce(TcpStream) + Send + 'static) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        request_head(&mut socket);
        work(socket);
    });
    (url, thread)
}

#[test]
fn headers_arrive_before_body_and_reads_decode_chunked_framing() {
    for transport in transports() {
        let (release, released) = mpsc::channel();
        let (url, peer) = server(move |mut socket| {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .unwrap();
            released.recv_timeout(Duration::from_secs(3)).unwrap();
            socket
                .write_all(b"3\r\none\r\n3;extension=yes\r\ntwo\r\n0\r\nX-End: yes\r\n\r\n")
                .unwrap();
        });
        let response = transport
            .open(&Request::get(&url), &AbortSignal::default())
            .unwrap();
        assert_eq!(response.status, 200);
        release.send(()).unwrap();
        assert_eq!(response.body.collect().unwrap(), b"onetwo");
        peer.join().unwrap();
    }
}

#[test]
fn abort_interrupts_waiting_for_headers_and_closes_the_socket() {
    for transport in transports() {
        let (arrived, arrival) = mpsc::channel();
        let (url, peer) = server(move |mut socket| {
            arrived.send(()).unwrap();
            assert_eq!(socket.read(&mut [0]).unwrap(), 0);
        });
        let control = AbortController::new();
        let signal = control.signal();
        let (done, completion) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(transport.open(&Request::get(&url), &signal).map(|_| ()))
                .unwrap();
        });
        arrival.recv_timeout(Duration::from_secs(3)).unwrap();
        control.abort();
        assert!(completion
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_err());
        worker.join().unwrap();
        peer.join().unwrap();
    }
}

#[test]
fn abort_interrupts_body_reads_and_drop_closes_an_unread_body() {
    for transport in transports() {
        for abort in [true, false] {
            let (url, peer) = server(move |mut socket| {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n")
                    .unwrap();
                assert_eq!(socket.read(&mut [0]).unwrap(), 0);
            });
            let control = AbortController::new();
            let response = transport
                .open(&Request::get(&url), &control.signal())
                .unwrap();
            if abort {
                let (entered, entry) = mpsc::channel();
                let (done, completion) = mpsc::channel();
                let worker = std::thread::spawn(move || {
                    let mut body = response.body;
                    entered.send(()).unwrap();
                    done.send(body.read(&mut [0; 10])).unwrap();
                });
                entry.recv_timeout(Duration::from_secs(3)).unwrap();
                control.abort();
                assert!(completion
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .is_err());
                worker.join().unwrap();
            } else {
                drop(response);
            }
            peer.join().unwrap();
        }
    }
}

#[test]
fn truncated_length_and_chunked_bodies_fail_instead_of_clean_eof() {
    for transport in transports() {
        for wire in [
            &b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nshort"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n8\r\nshort"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nyes\r\n"[..],
        ] {
            let (url, peer) = server(move |mut socket| {
                socket.write_all(wire).unwrap();
            });
            let response = transport
                .open(&Request::get(&url), &AbortSignal::default())
                .unwrap();
            assert!(response.body.collect().is_err());
            peer.join().unwrap();
        }
    }
}

#[test]
fn quota_applies_to_declared_and_streamed_bytes_but_not_head_metadata() {
    for transport in transports() {
        for wire in [
            &b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n"[..],
        ] {
            let (url, peer) = server(move |mut socket| { socket.write_all(wire).unwrap(); });
            let mut request = Request::get(&url);
            request.max_body = Some(4);
            let result = transport.open(&request, &AbortSignal::default()).and_then(|r| r.collect());
            assert!(result.unwrap_err().to_string().contains("4-byte limit"));
            peer.join().unwrap();
        }
        let (url, peer) = server(move |mut socket| {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n")
                .unwrap();
        });
        let mut request = Request::get(&url);
        request.method = "HEAD".into();
        request.max_body = Some(0);
        assert!(transport.send(&request).unwrap().body.is_empty());
        peer.join().unwrap();
    }
}

#[test]
fn pooled_connection_refreshes_cancellation_and_old_signal_cannot_close_new_request() {
    let transport = RustlsHttpTransport::new();
    let (release, released) = mpsc::channel();
    let (url, peer) = server(move |mut socket| {
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none")
            .unwrap();
        request_head(&mut socket); // A second TCP connection would leave this blocked.
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n")
            .unwrap();
        released.recv_timeout(Duration::from_secs(3)).unwrap();
        socket.write_all(b"two").unwrap();
        request_head(&mut socket);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n")
            .unwrap();
        assert_eq!(socket.read(&mut [0]).unwrap(), 0);
    });
    let first = AbortController::new();
    assert_eq!(
        transport
            .open(&Request::get(&url), &first.signal())
            .unwrap()
            .body
            .collect()
            .unwrap(),
        b"one"
    );
    let second = AbortController::new();
    let response = transport
        .open(&Request::get(&url), &second.signal())
        .unwrap();
    first.abort();
    release.send(()).unwrap();
    assert_eq!(response.body.collect().unwrap(), b"two");
    let third = AbortController::new();
    let mut response = transport
        .open(&Request::get(&url), &third.signal())
        .unwrap();
    second.abort();
    third.abort();
    assert!(response.body.read(&mut [0; 3]).is_err());
    peer.join().unwrap();
}

#[test]
fn an_unread_response_does_not_hold_up_another_request() {
    for transport in transports() {
        let (url, peer) = server(move |mut socket| {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\n")
                .unwrap();
            // No producer is installed to collect this body in the background.
            assert_eq!(socket.read(&mut [0]).unwrap(), 0);
        });
        let response = transport
            .open(&Request::get(&url), &AbortSignal::default())
            .unwrap();
        let (other_url, other_peer) = server(move |mut socket| {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
        });
        assert_eq!(
            transport.send(&Request::get(&other_url)).unwrap().body,
            b"ok"
        );
        drop(response);
        other_peer.join().unwrap();
        peer.join().unwrap();
    }
}
