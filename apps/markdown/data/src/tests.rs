use super::*;

fn reply(value: Json) -> Outcome {
    Outcome::Storage(serde_json::to_vec(&value).unwrap())
}

/// The storage request an answer makes: its op and path.
fn asked_for(answer: &Answer) -> (String, String) {
    let Answer::Later(request) = answer else {
        panic!("expected a request")
    };
    let payload: Json = serde_json::from_slice(request.storage.as_ref().unwrap()).unwrap();
    (
        payload["op"].as_str().unwrap().to_owned(),
        payload["args"]["path"].as_str().unwrap().to_owned(),
    )
}

fn step(source: &mut Markdown, asked: &str, outcome: Outcome) -> Answer {
    source
        .parse(&mut Store::default(), "open", &[Value::str(asked)], outcome)
        .unwrap()
}

fn fields(value: &Value) -> Vec<Value> {
    let Value::Record(fields) = value else {
        panic!("expected a document")
    };
    fields.to_vec()
}

#[test]
fn a_folder_opens_its_readme_through_storage_with_the_list_beside_it() {
    let mut source = Markdown::new();
    let folder = "doc:/3/notes";
    let first = source
        .answer(&mut Store::default(), "open", &[Value::str(folder)])
        .unwrap();
    assert_eq!(asked_for(&first), ("fs.stat".into(), folder.into()));
    let next = step(
        &mut source,
        folder,
        reply(json!({"size":0,"isFile":false,"isDirectory":true,"modifiedMs":0})),
    );
    assert_eq!(asked_for(&next), ("fs.readdir".into(), folder.into()));
    let next = step(
        &mut source,
        folder,
        reply(json!(["README.md", "b.md", "img.png"])),
    );
    let file = "doc:/3/notes/README.md";
    assert_eq!(asked_for(&next), ("fs.readFile".into(), file.into()));
    let text = "# Notes\n\nSee [b](b.md) and [out](../../x.md).";
    let next = step(
        &mut source,
        folder,
        reply(json!({"base64": exact_runner::agent::base64(text.as_bytes())})),
    );
    assert_eq!(asked_for(&next), ("fs.readdir".into(), folder.into()));
    let next = step(
        &mut source,
        folder,
        reply(json!(["README.md", "b.md", "img.png"])),
    );
    let Answer::Later(request) = next else {
        panic!("the parse goes to the worker")
    };
    let token = request.continuation.unwrap();
    let outcome = source.continuation(token).unwrap()();
    let Answer::Now(value) = step(&mut source, folder, outcome) else {
        panic!("expected the document")
    };
    let f = fields(&value);
    assert_eq!(f[0].as_str(), Some(file));
    assert_eq!(f[1].as_str(), Some("README.md"));
    assert_eq!(f[2].as_str(), Some("Notes"));
    assert_eq!(f[4].as_bool(), Some(false));
    let Value::List(siblings) = &f[7] else {
        panic!("expected siblings")
    };
    let names: Vec<_> = siblings
        .iter()
        .map(|s| fields(s)[2].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["doc:/3/notes/README.md", "doc:/3/notes/b.md"]);
    assert_eq!(resolve("doc:/3/notes", "b.md"), "doc:/3/notes/b.md");
    assert_eq!(resolve("doc:/3/notes", "../x.md#top"), "doc:/3/x.md");
    assert_eq!(
        resolve("doc:/3/notes", "../../../y.md"),
        "doc:/y.md",
        "names no document"
    );
    assert_eq!(
        resolve("doc:/3/notes", "https://a.example/x"),
        "https://a.example/x"
    );
}

#[test]
fn refusals_keep_the_open_document_and_a_path_never_minted_is_refused() {
    let mut source = Markdown::new();
    let file = "doc:/1/one.md";
    let answer = source
        .answer(&mut Store::default(), "open", &[Value::str(file)])
        .unwrap();
    assert_eq!(asked_for(&answer).0, "fs.stat");
    step(
        &mut source,
        file,
        reply(json!({"size":10,"isFile":true,"isDirectory":false,"modifiedMs":0})),
    );
    // A file chosen alone: no folder to list, straight to the parse.
    let answer = step(
        &mut source,
        file,
        reply(json!({"base64": exact_runner::agent::base64(b"# One")})),
    );
    let Answer::Later(request) = answer else {
        panic!("expected the worker")
    };
    let outcome = source.continuation(request.continuation.unwrap()).unwrap()();
    let Answer::Now(opened) = step(&mut source, file, outcome) else {
        panic!("expected the document")
    };
    assert_eq!(fields(&opened)[2].as_str(), Some("One"));
    assert_eq!(fields(&opened)[7], Value::list(Vec::new()));

    let Answer::Now(refused) = source
        .answer(&mut Store::default(), "open", &[Value::str("/etc/passwd")])
        .unwrap()
    else {
        panic!("a raw path is refused at once")
    };
    let r = fields(&refused);
    assert_eq!(
        r[3].as_str(),
        Some("/etc/passwd is not a document you opened")
    );
    assert_eq!(r[2].as_str(), Some("One"), "the open document stays");

    let big = "doc:/1/big.md";
    source
        .answer(&mut Store::default(), "open", &[Value::str(big)])
        .unwrap();
    let Answer::Now(refused) = step(
        &mut source,
        big,
        reply(json!({"size": LIMIT + 1, "isFile": true, "isDirectory": false, "modifiedMs": 0})),
    ) else {
        panic!("too large is refused")
    };
    assert!(fields(&refused)[3]
        .as_str()
        .unwrap()
        .contains("larger than this reader opens"));
    let missing = "doc:/9/gone.md";
    source
        .answer(&mut Store::default(), "open", &[Value::str(missing)])
        .unwrap();
    let Answer::Now(refused) = step(
        &mut source,
        missing,
        reply(json!({"error": "doc:/9/gone.md: no such document"})),
    ) else {
        panic!("a storage error is a refusal")
    };
    assert_eq!(
        fields(&refused)[3].as_str(),
        Some("doc:/9/gone.md: no such document")
    );
}

#[test]
fn a_running_parse_releases_the_lane_for_the_newest_file() {
    let mut source = Markdown::new();
    let old = "doc:/2/old.md";
    source
        .answer(&mut Store::default(), "open", &[Value::str(old)])
        .unwrap();
    step(
        &mut source,
        old,
        reply(json!({"size":1,"isFile":true,"isDirectory":false,"modifiedMs":0})),
    );
    let text = "[x".repeat(512 * 1024);
    let Answer::Later(request) = step(
        &mut source,
        old,
        reply(json!({"base64": exact_runner::agent::base64(text.as_bytes())})),
    ) else {
        panic!("expected the worker")
    };
    let stale = source.continuation(request.continuation.unwrap()).unwrap();
    source
        .answer(
            &mut Store::default(),
            "open",
            &[Value::str("doc:/2/new.md")],
        )
        .unwrap();
    assert!(matches!(
        stale(),
        Outcome::Failed {
            kind: FailureKind::Aborted,
            ..
        }
    ));
    assert!(source.done.lock().unwrap().is_empty());
}
