//! Engine-free FormData through the same Fetch value a Rust consumer holds.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use ibex2::grant::{Grant, GrantSet, Origin};
use ibex2::host::Host;
use ibex2::stdlib::fetch::Request;
use ibex2::stdlib::multipart::{EncodedMultipart, FormData};
use ibex2::transport::DevTcpTransport;

#[test]
fn rust_form_data_reaches_fetch_as_the_exact_multipart_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
            assert!(head.len() < 64 * 1024);
        }
        let head = String::from_utf8(head).unwrap();
        let header = |wanted: &str| {
            head.split("\r\n")
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
                .map(|(_, value)| value.trim().to_string())
                .unwrap()
        };
        let length: usize = header("content-length").parse().unwrap();
        let content_type = header("content-type");
        let boundary = content_type
            .strip_prefix("multipart/form-data; boundary=")
            .unwrap()
            .trim_matches('"')
            .to_string();
        let mut body = vec![0; length];
        stream.read_exact(&mut body).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        (head, boundary, body)
    });

    let mut form = FormData::new();
    form.append_text("field", "hello");
    form.append_file("file", b"a\0b".to_vec(), "snow-雪.bin", "application/test");
    let expected_form = form.clone();
    let mut request = Request::get(&format!("http://127.0.0.1:{port}/upload")).with_body(form);
    request.method = "POST".into();
    let grants = GrantSet::none().with(Grant::Fetch(Origin::new("http", "127.0.0.1", port)));
    let bindings = Host::with_transport(Box::new(DevTcpTransport::new())).endow(grants);
    assert_eq!(bindings.fetch.send(request).unwrap().text(), "ok");

    let (head, boundary, body) = server.join().unwrap();
    assert!(head.starts_with("POST /upload HTTP/1.1\r\n"));
    let expected = EncodedMultipart::with_boundary(&expected_form, boundary).unwrap();
    assert_eq!(body, expected.as_bytes());
}
