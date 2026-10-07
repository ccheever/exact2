use super::*;
use crate::stdlib::abort::AbortController;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn server(handler: impl FnOnce(TcpStream) + Send + 'static) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        handler(stream);
    });
    (url, worker)
}

#[test]
fn headers_arrive_before_body_completes_and_abort_interrupts_read() {
    let (release, wait) = mpsc::channel();
    let (url, server) = server(move |mut stream| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 5\r\n\r\nx")
            .unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte).unwrap(),
            0,
            "abort did not close task connection"
        );
    });
    let transport = DarwinTransport::new();
    let controller = AbortController::new();
    let mut response = transport
        .open(&Request::get(&url), &controller.signal())
        .unwrap();
    assert_eq!(response.status, 200);
    // CFNetwork waits for an initial byte before its header callback, even
    // with an explicit Content-Type and nosniff. The rest is still streaming.
    assert_eq!(response.body.read(&mut [0; 1]).unwrap(), 1);
    let (done, result) = mpsc::channel();
    let reader = thread::spawn(move || {
        done.send(response.body.read(&mut [0; 5])).unwrap();
    });
    controller.abort();
    assert!(result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("AbortError"));
    release.send(()).unwrap();
    reader.join().unwrap();
    server.join().unwrap();
}

#[test]
fn abort_interrupts_wait_for_headers() {
    let (accepted, request) = mpsc::channel();
    let (url, server) = server(move |mut stream| {
        accepted.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let controller = AbortController::new();
    let signal = controller.signal();
    let (done, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        done.send(DarwinTransport::new().open(&Request::get(&url), &signal))
            .unwrap();
    });
    request.recv_timeout(Duration::from_secs(2)).unwrap();
    controller.abort();
    assert!(result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("AbortError"));
    worker.join().unwrap();
    server.join().unwrap();
}

#[test]
fn dropping_unread_body_closes_task() {
    let (url, server) = server(move |mut stream| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 999999\r\n\r\nx")
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let response = DarwinTransport::new()
        .open(&Request::get(&url), &AbortSignal::default())
        .unwrap();
    drop(response);
    server.join().unwrap();
}

#[test]
fn slow_large_body_does_not_block_another_response() {
    const SIZE: usize = 8 * 1024 * 1024;
    let (sent, sending) = mpsc::channel();
    let (url, server) = server(move |mut stream| {
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {SIZE}\r\n\r\n").unwrap();
        sent.send(()).unwrap();
        for _ in 0..SIZE / 16384 {
            stream.write_all(&[42; 16384]).unwrap();
        }
    });
    let transport = DarwinTransport::new();
    let mut response = transport
        .open(&Request::get(&url), &AbortSignal::default())
        .unwrap();
    sending.recv_timeout(Duration::from_secs(2)).unwrap();
    // Give the producer enough time to fill transport and socket buffers while
    // this response is unread. A second exchange must still make progress.
    thread::sleep(Duration::from_millis(100));
    let (other, second) = server_fn_small();
    assert_eq!(transport.send(&Request::get(&other)).unwrap().body, b"ok");
    let mut total = 0;
    let mut bytes = [0; 4096];
    loop {
        let n = response.body.read(&mut bytes).unwrap();
        if n == 0 {
            break;
        }
        assert!(bytes[..n].iter().all(|&v| v == 42));
        total += n;
        if total % (256 * 1024) == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }
    assert_eq!(total, SIZE);
    server.join().unwrap();
    second.join().unwrap();
}
fn server_fn_small() -> (String, thread::JoinHandle<()>) {
    server(|mut stream| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
    })
}

#[test]
fn declared_length_over_limit_is_rejected_before_body_is_exposed() {
    let (url, server) = server(|mut stream| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 1000\r\n\r\nx")
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let mut request = Request::get(&url);
    request.max_body = Some(10);
    let error = DarwinTransport::new()
        .open(&request, &AbortSignal::default())
        .unwrap_err();
    assert!(error.to_string().contains("10-byte limit"));
    server.join().unwrap();
}

#[test]
fn fifth_session_wait_is_cancellable_and_drop_returns_a_lease() {
    let transport = Arc::new(DarwinTransport::new());
    let mut responses = Vec::new();
    let mut servers = Vec::new();
    for _ in 0..4 {
        let (url, worker) = server(|mut stream| {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 5\r\n\r\nx")
                .unwrap();
            assert_eq!(stream.read(&mut [0]).unwrap(), 0);
        });
        responses.push(
            transport
                .open(&Request::get(&url), &AbortSignal::default())
                .unwrap(),
        );
        servers.push(worker);
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let controller = AbortController::new();
    let signal = controller.signal();
    let clone = transport.clone();
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        send.send(clone.open(&Request::get(&url), &signal)).unwrap();
    });
    thread::sleep(Duration::from_millis(50));
    assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
    controller.abort();
    assert!(receive
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("AbortError"));
    worker.join().unwrap();
    drop(responses.pop());
    let (url, small) = server_fn_small();
    assert_eq!(transport.send(&Request::get(&url)).unwrap().body, b"ok");
    drop(responses);
    for server in servers {
        server.join().unwrap();
    }
    small.join().unwrap();
}

#[test]
fn completed_body_returns_connection_and_unregisters_old_abort() {
    let (url, server) = server(|mut stream| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
    });
    let transport = DarwinTransport::new();
    let controller = AbortController::new();
    let response = transport
        .open(&Request::get(&url), &controller.signal())
        .unwrap();
    assert_eq!(response.collect().unwrap().body, b"ok");
    controller.abort();
    assert_eq!(transport.send(&Request::get(&url)).unwrap().body, b"ok");
    assert_eq!(transport.last_connection_was_reused(), Some(true));
    server.join().unwrap();
}

#[test]
fn completed_unread_bodies_do_not_exhaust_session_pool() {
    let transport = DarwinTransport::new();
    let old = AbortController::new();
    let mut responses = Vec::new();
    let mut servers = Vec::new();
    for _ in 0..8 {
        let (url, worker) = server_fn_small();
        responses.push(transport.open(&Request::get(&url), &old.signal()).unwrap());
        servers.push(worker);
    }
    let (url, worker) = server_fn_small();
    let final_response = transport
        .open(&Request::get(&url), &AbortSignal::default())
        .unwrap();
    // Cancelling completed old exchanges cannot cancel the session's next task.
    old.abort();
    assert_eq!(final_response.collect().unwrap().body, b"ok");
    for response in responses {
        assert!(response.collect().is_err());
    }
    for server in servers {
        server.join().unwrap();
    }
    worker.join().unwrap();
}

#[test]
fn head_representation_length_does_not_count_against_body_limit() {
    let (url, server) = server(|mut stream| {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let mut request = Request::get(&url);
    request.method = "HEAD".into();
    request.max_body = Some(0);
    let response = DarwinTransport::new().send(&request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.is_empty());
    assert_eq!(response.headers.get("content-length"), Some("100"));
    server.join().unwrap();
}

#[test]
fn not_modified_representation_length_does_not_count_against_body_limit() {
    let (url, server) = server(|mut stream| {
        stream
            .write_all(
                b"HTTP/1.1 304 Not Modified\r\nContent-Length: 100\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
    });
    let mut request = Request::get(&url);
    request.max_body = Some(0);
    let response = DarwinTransport::new().send(&request).unwrap();
    assert_eq!(response.status, 304);
    assert!(response.body.is_empty());
    assert_eq!(response.headers.get("content-length"), Some("100"));
    server.join().unwrap();
}
