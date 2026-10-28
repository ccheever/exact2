//! @ref LLP 1133.024 D2, D3 — a received share is one location, and its
//! files arrive by token after its text.
use exact_kernel::{Kernel, PropValue};
use exact_runner::share_in::{self, Claims, Pending, PendingFile, ShareTarget, SharedFiles};
use exact_runner::time::{launching_with, Launch};
use exact_runner::{DataError, DataSource, Picked, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const SOURCE: &str = r#"routes nav
  tab home "/"
    share "/share"
  notfound
component App
  derive here = top(nav)
  derive files = sharedFiles(here, "files")
  action follow(url: string)
    nav = go(nav, url)
  action back
    nav = back(nav)
  view
    column navigationKey=`${here.id}` navigationBack="back" navigate=follow
      text (here.name == "share" ? searchParam(here, "text") : "home") testId="text"
      text `${files.files.length} ${files.pending} ${files.dropped}` testId="counts"
      for f in files.files key=f.path
        text `${f.name} ${f.type} ${f.size} ${f.path}` testId="file"
"#;

fn text_of(r: &Runner<NoData>, id: &str) -> Vec<String> {
    let k = r.kernel();
    k.find_by_test_id(id)
        .into_iter()
        .map(|key| {
            k.node_by_key(key)
                .unwrap()
                .props
                .iter()
                .find_map(|(p, v)| match v {
                    PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                    _ => None,
                })
                .unwrap()
        })
        .collect()
}

fn boot(launch: Launch, location: &str) -> Runner<NoData> {
    let plan = contract::compile(SOURCE).unwrap();
    launching_with(launch, || {
        Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            location,
        )
    })
    .unwrap()
}

fn navigate(r: &mut Runner<NoData>, location: &str) {
    let root = r.kernel().roots()[0];
    r.dispatch(root, exact_runner::Event::Navigate(location.into()))
        .unwrap();
}

fn target() -> ShareTarget {
    ShareTarget {
        action: "/share".into(),
        title: Some("title".into()),
        text: Some("text".into()),
        url: Some("url".into()),
        files: vec![("files".into(), vec!["text/plain".into(), ".md".into()])],
    }
}

#[test]
fn a_cold_share_shows_its_text_first_with_its_files_pending_then_they_land() {
    let dir = std::env::temp_dir().join(format!("exact-share-{}", std::process::id()));
    let inbox = dir.join("inbox");
    let shared = dir.join("shared");
    std::fs::create_dir_all(&inbox).unwrap();
    std::fs::write(inbox.join("1.txt"), "first file").unwrap();
    std::fs::write(inbox.join("2.md"), "# second").unwrap();
    std::fs::write(inbox.join("3.bin"), [0u8; 4]).unwrap();
    let share = Pending {
        text: Some("hello there".into()),
        files: ["1.txt", "2.md", "3.bin"]
            .map(|n| PendingFile {
                source: inbox.join(n),
                name: format!("from sender {n}"),
                mime: None,
                member: None,
                size: None,
            })
            .to_vec(),
        ..Pending::default()
    };
    let mut claims = Claims::new(7.0);
    let mut claimed = claims.claim(&target(), share);
    let token = share_in::token(7.0, 0);
    assert_eq!(
        claimed.location,
        format!("/share?text=hello%20there&files={token}")
    );
    assert_eq!(
        claimed.line,
        "share: received text; files: text/plain 10 B, text/markdown 8 B"
    );
    // The first frame is the share's route with its text, the two accepted files pending and one dropped.
    let launch = Launch {
        shared: claimed.tokens.clone(),
        claims: Some(claims.clone()),
        ..Launch::NONE
    };
    let mut r = boot(launch, &claimed.location);
    assert_eq!(text_of(&r, "text"), ["hello there"]);
    assert_eq!(text_of(&r, "counts"), ["0 2 1"]);
    // The copies land: the files answer under their stored names, never the sender's.
    for (token, files) in claimed.copy(&shared, true) {
        r.set_shared(&token, files).unwrap();
    }
    assert_eq!(text_of(&r, "counts"), ["2 0 1"]);
    let files = text_of(&r, "file");
    assert_eq!(
        files[0],
        "from sender 1.txt text/plain 10 app:/tmp/shared/7-1.txt"
    );
    assert_eq!(
        files[1],
        "from sender 2.md text/markdown 8 app:/tmp/shared/7-2.md"
    );
    assert_eq!(
        std::fs::read_to_string(shared.join("7-1.txt")).unwrap(),
        "first file"
    );
    assert!(
        !inbox.join("1.txt").exists(),
        "a claim moves the inbox's file"
    );
    // A share later this launch draws past the cold one's token and names.
    std::fs::write(inbox.join("4.txt"), "later").unwrap();
    let later = Pending {
        files: vec![PendingFile {
            source: inbox.join("4.txt"),
            name: "later.txt".into(),
            mime: None,
            member: None,
            size: None,
        }],
        ..Pending::default()
    };
    let mut again = r.claim_share(&target(), later).unwrap();
    assert_eq!(
        again.location,
        format!("/share?files={}", share_in::token(7.0, 1))
    );
    for (token, files) in again.tokens.clone() {
        r.set_shared(&token, files).unwrap();
    }
    navigate(&mut r, &again.location);
    assert_eq!(text_of(&r, "counts"), ["0 1 0"]);
    for (token, files) in again.copy(&shared, false) {
        r.set_shared(&token, files).unwrap();
    }
    assert_eq!(
        text_of(&r, "file"),
        ["later.txt text/plain 5 app:/tmp/shared/7-3.txt"]
    );
    assert!(inbox.join("4.txt").exists(), "a copy leaves its source");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_warm_share_navigates_and_a_token_not_set_this_launch_answers_none() {
    let mut r = boot(Launch::NONE, "/");
    assert_eq!(text_of(&r, "text"), ["home"]);
    // A written link, or an earlier launch's token, shows an empty share and says so.
    navigate(&mut r, "/share?text=old&files=ABCDEFGHJKMN");
    assert_eq!(text_of(&r, "text"), ["old"]);
    assert_eq!(text_of(&r, "counts"), ["0 0 0"]);
    assert!(r
        .journal()
        .any(|l| l.contains("no share under `files` this launch")));
    // A token set while the route shows it is read at once, pending then landed.
    let token = share_in::token(1.0, 3);
    navigate(&mut r, &format!("/share?text=new&files={token}"));
    r.set_shared(
        &token,
        SharedFiles {
            files: Vec::new(),
            dropped: 0,
            pending: 1,
        },
    )
    .unwrap();
    assert_eq!(text_of(&r, "counts"), ["0 1 0"]);
    let picked = Picked {
        path: "app:/tmp/shared/1-1.txt".into(),
        name: "a.txt".into(),
        mime: "text/plain".into(),
        size: 3.0,
        width: None,
        height: None,
        duration: None,
    };
    r.set_shared(
        &token,
        SharedFiles {
            files: vec![picked],
            dropped: 0,
            pending: 0,
        },
    )
    .unwrap();
    assert_eq!(text_of(&r, "counts"), ["1 0 0"]);
    // A path outside app:/tmp/shared/ is refused and changes nothing.
    let mut outside = SharedFiles::default();
    outside.files.push(Picked {
        path: "app:/data/secret.db".into(),
        name: "x".into(),
        mime: "x".into(),
        size: 1.0,
        width: None,
        height: None,
        duration: None,
    });
    assert!(r.set_shared(&token, outside).unwrap().is_none());
    assert_eq!(text_of(&r, "counts"), ["1 0 0"]);
}

#[test]
fn the_manifest_and_the_inbox_record_read_as_the_rfc_writes_them() {
    let member = serde_json::json!({
        "action": "/share", "method": "POST", "enctype": "multipart/form-data",
        "params": { "title": "title", "text": "text", "files": [{ "name": "files", "accept": ["text/plain", ".md"] }] }
    });
    let t = ShareTarget::from_json(&member).unwrap();
    assert_eq!(
        t.files,
        vec![(
            "files".to_string(),
            vec!["text/plain".to_string(), ".md".to_string()]
        )]
    );
    assert_eq!(t.url, None);
    let dir = std::path::Path::new("/inbox/abc");
    let record = serde_json::json!({ "v": 1, "text": "hi", "url": "https://x.test", "files": [{ "file": "1.txt", "name": "Notes.txt", "type": "text/plain", "member": "files" }], "dropped": { "files": 2 } });
    let p = Pending::from_json(&record, Some(dir)).unwrap();
    assert_eq!(p.files[0].source, dir.join("1.txt"));
    assert_eq!(p.dropped.get("files"), Some(&2));
    // A stored name that leaves the share's directory is refused, and so is another version.
    for bad in ["../x", "a/b", "share.json", ""] {
        let record = serde_json::json!({ "v": 1, "files": [{ "file": bad }] });
        assert!(Pending::from_json(&record, Some(dir)).is_none(), "{bad}");
    }
    assert!(Pending::from_json(&serde_json::json!({ "v": 2 }), Some(dir)).is_none());
    // With no `url` member mapped, a link goes to the text.
    let mut claims = Claims::new(1.0);
    let claimed = claims.claim(&t, p);
    assert!(
        claimed
            .location
            .starts_with("/share?text=hi%0Ahttps%3A%2F%2Fx.test&files="),
        "{}",
        claimed.location
    );
}

#[test]
fn the_bake_refuses_a_share_target_no_route_matches() {
    let dir = std::env::temp_dir().join(format!("exact2-share-target-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.contract"), SOURCE).unwrap();
    let compile = |action: &str| {
        let manifest = format!(
            r#"{{"app": {{"id": "dev.test.share", "name": "Share"}}, "share_target": {{"action": "{action}", "params": {{"text": "text"}}}}}}"#
        );
        std::fs::write(dir.join("app.json"), manifest).unwrap();
        contract::compile_path_all(&dir.join("app.contract"), false)
            .map(drop)
            .map_err(|e| e.into_iter().map(|e| (e.id, e.message)).collect::<Vec<_>>())
    };
    assert!(compile("/share").is_ok());
    assert!(compile("/share?from=sheet").is_ok());
    let refused = compile("/inbox").unwrap_err();
    assert_eq!(refused[0].0, "bake-share-target");
    assert!(
        refused[0]
            .1
            .contains("matches none of the app's routes (/, /share)"),
        "{}",
        refused[0].1
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
