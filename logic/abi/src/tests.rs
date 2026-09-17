use super::*;

struct Fixture(u32);
impl DataSource for Fixture {
    fn app_id(&self) -> &str {
        "test.logic"
    }
    fn grants(&self) -> &str {
        "secret.keep token"
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(self.0.into()))
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        self.0 += 1;
        let old = store.get("token").unwrap_or("absent").to_string();
        store.set("token", "first")?;
        store.forget("token")?;
        store.set("token", "last")?;
        match source {
            "network" => Ok(Answer::Later(Request::post_json(
                "https://example.com/",
                &old,
            ))),
            "continuation" => Ok(Answer::Later(Request::continuation(1))),
            "storage" | "storage-empty-scope" | "storage-scoped" => {
                let mut request = Request::storage(
                    br#"{"version":1,"op":"fs.readFile","args":{"path":"app:/data/note"}}"#
                        .to_vec(),
                );
                request.grants = match source {
                    "storage-empty-scope" => Some(String::new()),
                    "storage-scoped" => Some("fs.read app:/data/note".into()),
                    _ => None,
                };
                Ok(Answer::Later(request))
            }
            _ => Ok(Answer::Now(Value::record(vec![
                Value::str(&old),
                Value::Number(self.0.into()),
            ]))),
        }
    }
    fn parse(
        &mut self,
        store: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        store.observe_external_read();
        match outcome {
            Outcome::Response(r) => Ok(Answer::Now(Value::Number(r.status.into()))),
            Outcome::Storage(bytes) => Ok(Answer::Now(Value::list(
                bytes
                    .into_iter()
                    .map(|byte| Value::Number(byte.into()))
                    .collect(),
            ))),
            Outcome::Failed { message, .. } => Err(DataError::Unavailable(message)),
        }
    }
}

#[test]
fn portable_storage_and_empty_scopes_round_trip_without_becoming_http() {
    let mut module = Session::new(Fixture(0));
    let mut store = Store::new("secret.keep token", []);
    for (source, scope) in [
        ("storage", None),
        ("storage-empty-scope", Some("")),
        ("storage-scoped", Some("fs.read app:/data/note")),
    ] {
        module
            .dispatch(&call_request(&store, source, &[], None).unwrap())
            .unwrap();
        let Answer::Later(request) = call_reply(module.output(), &mut store).unwrap() else {
            panic!("expected storage request")
        };
        assert_eq!(
            request.storage,
            Some(br#"{"version":1,"op":"fs.readFile","args":{"path":"app:/data/note"}}"#.to_vec())
        );
        assert_eq!(request.grants.as_deref(), scope);
        assert!(request.url.is_empty() && request.continuation.is_none());
    }
    let outcome = Outcome::Storage(vec![0, 128, 255]);
    module
        .dispatch(&call_request(&store, "storage", &[], Some(&outcome)).unwrap())
        .unwrap();
    assert_eq!(
        call_reply(module.output(), &mut store).unwrap(),
        Answer::Now(Value::list(vec![
            Value::Number(0.),
            Value::Number(128.),
            Value::Number(255.)
        ]))
    );
}

#[test]
fn network_request_preserves_absent_empty_and_restricted_grants() {
    for scope in [None, Some(""), Some("net.fetch https://example.com/")] {
        let mut request = Request::get("https://example.com/");
        request.grants = scope.map(str::to_string);
        let mut encoded = Writer::default();
        encode_result(&mut encoded, Ok(Answer::Later(request.clone())));
        let bytes = encoded.into_vec();
        let mut decoded = Reader::new(&bytes);
        assert_eq!(
            read_result(&mut decoded).unwrap(),
            Ok(Answer::Later(request))
        );
        end(&decoded).unwrap();
    }
}

#[test]
fn storage_unknown_tags_truncation_and_oversize_are_refused_before_effects() {
    for tag in [6, 255] {
        assert!(read_outcome(&mut Reader::new(&[tag])).is_err());
        assert!(read_result(&mut Reader::new(&[tag])).is_err());
    }
    let mut invalid = Writer::default();
    invalid.u8(5);
    bytes(&mut invalid, b"{}");
    invalid.u8(2);
    invalid.string("");
    assert!(read_result(&mut Reader::new(&invalid.into_vec())).is_err());
    let mut truncated = Writer::default();
    truncated.u8(5);
    truncated.u32(4);
    truncated.bytes(&[0]);
    assert!(read_outcome(&mut Reader::new(&truncated.into_vec())).is_err());
    let mut module = Session::new(Fixture(0));
    let oversized = vec![0; MAX_MESSAGE + 1];
    assert!(module.dispatch(&oversized).is_err());
    assert!(call_request(
        &Store::default(),
        "storage",
        &[],
        Some(&Outcome::Storage(oversized.clone()))
    )
    .is_err());
    let mut encoded = header(2);
    encoded.u8(0);
    encoded.u32(0);
    encode_result(&mut encoded, Ok(Answer::Later(Request::storage(oversized))));
    assert!(finish(encoded).is_err());
    let mut legacy = metadata_request();
    legacy[..4].copy_from_slice(&1u32.to_le_bytes());
    assert!(module.dispatch(&legacy).is_err());
}

#[test]
fn reserved_store_writes_are_refused_before_any_other_write_is_applied() {
    let mut encoded = header(2);
    encoded.u8(1);
    encoded.u32(2);
    for (name, value) in [("token", "first"), ("exact.kept.hidden", "not allowed")] {
        encoded.string(name);
        encoded.u8(1);
        encoded.string(value);
    }
    encode_result(&mut encoded, Ok(Answer::Now(Value::Unit)));
    let mut store = Store::new("secret.keep token\nsecret.keep exact.kept.hidden", []);
    assert!(call_reply(&encoded.into_vec(), &mut store).is_err());
    assert!(store.take_writes().is_empty());
    assert_eq!(store.reads(), 0);
}

#[test]
fn preserves_session_store_read_marker_and_write_order() {
    let mut module = Session::new(Fixture(0));
    let mut store = Store::new(
        "secret.keep token",
        [
            ("token".into(), "old".into()),
            ("exact.kept.hidden".into(), "private".into()),
        ],
    );
    let input = call_request(&store, "answer", &[], None).unwrap();
    assert!(!input.windows(7).any(|x| x == b"private"));
    module.dispatch(&input).unwrap();
    assert_eq!(
        call_reply(module.output(), &mut store).unwrap(),
        Answer::Now(Value::record(vec![Value::str("old"), Value::Number(1.)]))
    );
    assert_eq!(store.reads(), 1);
    let writes = store.take_writes();
    assert_eq!(
        writes
            .iter()
            .map(|x| x.value.as_deref())
            .collect::<Vec<_>>(),
        [Some("first"), None, Some("last")]
    );
    module
        .dispatch(&call_request(&store, "answer", &[], None).unwrap())
        .unwrap();
    assert_eq!(
        call_reply(module.output(), &mut store).unwrap(),
        Answer::Now(Value::record(vec![Value::str("last"), Value::Number(2.)]))
    );
}

#[test]
fn request_outcome_and_continuation_refusal_cross_the_seam() {
    let mut module = Session::new(Fixture(0));
    let mut store = Store::new("secret.keep token", []);
    module
        .dispatch(&call_request(&store, "network", &[], None).unwrap())
        .unwrap();
    assert_eq!(
        call_reply(module.output(), &mut store).unwrap(),
        Answer::Later(Request::post_json("https://example.com/", "absent"))
    );
    for outcome in [
        Outcome::Response(Response {
            status: 201,
            headers: vec![("x".into(), "y".into())],
            body: vec![0, 128, 255],
        }),
        Outcome::Failed {
            kind: FailureKind::Refused,
            message: "refused".into(),
        },
    ] {
        let expected = if matches!(outcome, Outcome::Response(_)) {
            Ok(Answer::Now(Value::Number(201.)))
        } else {
            Err(DataError::Unavailable("refused".into()))
        };
        module
            .dispatch(&call_request(&store, "network", &[], Some(&outcome)).unwrap())
            .unwrap();
        assert_eq!(call_reply(module.output(), &mut store), expected);
    }
    module
        .dispatch(&call_request(&store, "continuation", &[], None).unwrap())
        .unwrap();
    assert!(
        matches!(call_reply(module.output(),&mut store),Err(DataError::Unavailable(s)) if s.contains("continuations"))
    );
}

#[test]
fn malformed_reply_cannot_partially_write_the_host_store() {
    let mut module = Session::new(Fixture(0));
    let mut store = Store::new("secret.keep token", []);
    module
        .dispatch(&call_request(&store, "answer", &[], None).unwrap())
        .unwrap();
    let mut reply = module.output().to_vec();
    reply.push(0);
    assert!(call_reply(&reply, &mut store).is_err());
    assert!(store.take_writes().is_empty());
    assert_eq!(store.reads(), 0);
    let mut ungranted = Store::default();
    assert!(call_reply(module.output(), &mut ungranted).is_err());
    assert!(ungranted.take_writes().is_empty());
}

mod exports {
    crate::export!(super::Fixture, super::Fixture(0));
}
#[test]
fn exported_buffers_belong_to_module_and_sessions_are_distinct() {
    assert_eq!(exports::exact_logic_abi(), ABI);
    let input = metadata_request();
    let a = exports::exact_logic_create();
    let b = exports::exact_logic_create();
    assert_ne!(a, b);
    let ptr = exports::exact_logic_alloc(input.len() as u32);
    unsafe {
        std::ptr::copy_nonoverlapping(input.as_ptr(), ptr as *mut u8, input.len());
        assert_eq!(exports::exact_logic_call(a, ptr, input.len() as u32), 0);
        exports::exact_logic_dealloc(ptr, input.len() as u32);
        let output = std::slice::from_raw_parts(
            exports::exact_logic_output(a) as *const u8,
            exports::exact_logic_output_len(a) as usize,
        )
        .to_vec();
        assert_eq!(
            metadata_reply(&output).unwrap(),
            ("test.logic".into(), "secret.keep token".into())
        );
        exports::exact_logic_destroy(a);
        exports::exact_logic_destroy(b);
    }
    assert_eq!(exports::exact_logic_alloc((MAX_MESSAGE + 1) as u32), 0);
}

#[test]
fn explicit_independent_http_survives_the_module_codec() {
    let request = Request::get("https://example.test/").independent_http(4096);
    let mut encoded = Writer::default();
    encode_result(&mut encoded, Ok(Answer::Later(request.clone())));
    let bytes = encoded.into_vec();
    let mut reader = Reader::new(&bytes);
    assert_eq!(
        read_result(&mut reader).unwrap(),
        Ok(Answer::Later(request))
    );
    end(&reader).unwrap();
}
