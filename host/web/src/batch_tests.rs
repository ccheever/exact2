//! Golden tests: each op's JSON as the batch writes it, byte for byte (the
//! texts were taken from the `format!`-built writer these replaced).

use crate::batch::Batch;
use exact_plan::Value;
use exact_runner::{HttpScheduling, Request, RequestOut, SurfaceRequest};

fn one(op: impl FnOnce(&mut Batch)) -> String {
    let mut batch = Batch::new();
    op(&mut batch);
    batch.finish(None, false, 0.0, None)
}

fn request(ticket: u64, request: Request, forced: bool) -> RequestOut {
    RequestOut {
        ticket,
        target: "feed \"home\"".into(),
        request,
        forced,
    }
}

fn cases() -> Vec<(&'static str, String)> {
    let mut http = Request::get("https://api.example/articles?tag=a b");
    http.method = "POST".into();
    http.headers = vec![
        ("content-type".into(), "application/json".into()),
        ("x".into(), "\"q\"".into()),
    ];
    http.body = b"{\"a\":1}".to_vec();
    http.grants = Some("net.fetch https://api.example".into());
    let mut independent = Request::get("https://api.example/big");
    independent.http = HttpScheduling::Independent {
        max_response_bytes: 1 << 20,
    };
    let mut capture = Request::get("");
    capture.surface = Some(Box::new(SurfaceRequest::Capture {
        name: "sketch".into(),
    }));
    capture.grants = Some("surface.carry sketch".into());
    let mut restore = Request::get("");
    restore.surface = Some(Box::new(SurfaceRequest::Restore {
        name: "sketch".into(),
        bytes: vec![1, 2, 3, 250],
    }));
    let mut mixed = Request::get("https://x.example/");
    mixed.surface = Some(Box::new(SurfaceRequest::Capture { name: "s".into() }));
    let mut storage = Request::storage(b"{\"op\":\"get\",\"key\":\"k\"}".to_vec());
    storage.grants = Some("store.local notes".into());
    let head = exact_runner::Head {
        title: Some("Home \"Conduit\"".into()),
        description: None,
        image: Some("/i.png".into()),
        canonical: None,
        robots: Some("noindex".into()),
        status: Some(404),
        edited: false,
    };
    let bare = exact_runner::Head::default();
    let surface = exact_runner::SurfaceUpdate {
        view: 31,
        name: "chart".into(),
        mode: exact_plan::SurfaceArgsMode::Named,
        names: vec!["points".into(), "label".into()],
        values: vec![
            Value::list(vec![Value::Number(1.5), Value::Number(-2.0)]),
            Value::str("a\"b"),
        ],
    };
    vec![
        (
            "router",
            one(|b| {
                b.router(&exact_runner::RouterChange {
                    top: 5,
                    url: "/tag/\"x\"?q=1".into(),
                    removed: vec![],
                })
            }),
        ),
        (
            "router-removed",
            one(|b| {
                b.router(&exact_runner::RouterChange {
                    top: 18_446_744_073_709_551_615,
                    url: "/".into(),
                    removed: vec![1, 22, 333],
                })
            }),
        ),
        ("collections", one(|b| b.collections("[{\"view\":1}]"))),
        ("refuse", one(|b| b.refuse(9, "no \"way\"\n"))),
        (
            "grants",
            one(|b| {
                b.grants("# reach\n\n net.fetch https://api.example \nsecret.keep app.token\nsurface.read \"sketch\"\n")
            }),
        ),
        // Crew's set (the port report of 2026-09-24, F1): one bad line, and
        // the page is handed nothing to admit.
        (
            "grants-unparsed",
            one(|b| b.grants("net.fetch https://crew.test\nsecret.keep crewHost")),
        ),
        ("textflow", one(|b| b.textflow("[]"))),
        (
            "create",
            one(|b| {
                b.create(
                    7,
                    "a",
                    &[
                        ("href", "/x".into()),
                        ("data-exact-on", "click input".into()),
                    ],
                    "color: red; content: \"\\\"",
                    &["click", "input"],
                )
            }),
        ),
        (
            "create-bare",
            one(|b| b.create(4_294_967_295, "div", &[], "", &[])),
        ),
        ("head", one(|b| b.head(&head))),
        ("head-bare", one(|b| b.head(&bare))),
        (
            "adopt",
            one(|b| {
                b.adopt(true);
                b.adopt(false)
            }),
        ),
        (
            "props",
            one(|b| b.props(3, &[("value", "a\tb".into())], &["checked", "disabled"])),
        ),
        ("props-bare", one(|b| b.props(0, &[], &[]))),
        ("style", one(|b| b.style(12, "width: 1.5px"))),
        (
            "children",
            one(|b| {
                b.children(1, &[]);
                b.children(2, &[5, 6, 4_294_967_295])
            }),
        ),
        (
            "animate",
            one(|b| {
                b.animate(
                    8,
                    "opacity",
                    0.0,
                    16.5,
                    &[[0.0; 4], [0.25, 1.0, 0.0, 0.0], [1e21, -0.0, 0.0, 0.0]],
                    false,
                )
            }),
        ),
        (
            "animate-pairs",
            one(|b| {
                b.animate(
                    9,
                    "translate",
                    12.25,
                    300.0,
                    &[[0.1, 0.2, 0.0, 0.0], [-3.5, 1e-7, 0.0, 0.0]],
                    true,
                )
            }),
        ),
        (
            "animate-stop",
            one(|b| b.animate(10, "scale", 0.0, 0.0, &[], false)),
        ),
        (
            "spring",
            one(|b| {
                b.spring(
                    2500.5,
                    9,
                    "translate",
                    (0.0, 300.0),
                    &[[0.1, 0.2, 0.0, 0.0], [0.0; 4]],
                )
            }),
        ),
        ("retire-motion", one(|b| b.retire_motion(11, "transform"))),
        ("timelines", one(|b| b.timelines())),
        ("surface", one(|b| b.surface(&surface))),
        ("request", one(|b| b.request(&request(41, http, true)))),
        (
            "request-independent",
            one(|b| b.request(&request(42, independent, false))),
        ),
        (
            "request-body-from",
            one(|b| {
                let mut upload =
                    Request::get("https://api.example/blob").body_from("app:/tmp/a.jpg");
                upload.method = "POST".into();
                b.request(&request(48, upload, false))
            }),
        ),
        (
            "request-continue",
            one(|b| {
                b.request(&request(
                    43,
                    Request::continuation(9_007_199_254_740_993),
                    false,
                ))
            }),
        ),
        (
            "request-storage",
            one(|b| b.request(&request(44, storage, false))),
        ),
        (
            "request-capture",
            one(|b| b.request(&request(45, capture, false))),
        ),
        (
            "request-restore",
            one(|b| b.request(&request(46, restore, false))),
        ),
        (
            "request-mixed",
            one(|b| b.request(&request(47, mixed, false))),
        ),
        ("destroy", one(|b| b.destroy(13))),
        (
            "at",
            one(|b| {
                b.at(16.5);
                b.at(0.1 + 0.2);
                b.at(-0.0)
            }),
        ),
        (
            "roots",
            one(|b| {
                b.roots(&[]);
                b.roots(&[1, 2, 30])
            }),
        ),
        (
            "command",
            one(|b| {
                b.command(
                    "copy",
                    &[
                        Value::Number(1.5),
                        Value::Number(f64::NAN),
                        Value::Bool(true),
                        Value::str("t"),
                        Value::Option(None),
                        Value::list(vec![Value::Number(3.0), Value::Unit]),
                    ],
                    Some(7),
                )
            }),
        ),
        ("finish-timers", {
            let mut b = Batch::new();
            b.destroy(1);
            b.accept_collection();
            b.finish(Some(16.0), false, 1234.5, Some("refused: \"x\""))
        }),
        ("finish-empty", Batch::new().finish(None, false, 0.0, None)),
        (
            "finish-fraction",
            Batch::new().finish(Some(0.1), false, 1e-7, None),
        ),
    ]
}

const GOLDEN: &[(&str, &str)] = &[
    (
        "router",
        r#"{"ops":[{"op":"router","top":5,"url":"/tag/\"x\"?q=1","removed":[]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "router-removed",
        r#"{"ops":[{"op":"router","top":18446744073709551615,"url":"/","removed":[1,22,333]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "collections",
        r#"{"ops":[{"op":"collections","items":[{"view":1}]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "refuse",
        r#"{"ops":[{"op":"refuse","ticket":9,"message":"no \"way\"\n"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "grants",
        r##"{"ops":[{"op":"grants","lines":["net.fetch https://api.example","secret.keep app.token","surface.read \"sketch\""],"set":{"version":1,"entries":[[1,"# reach",null,null],[3,"net.fetch https://api.example",["fetch","https","api.example",443],null],[4,"secret.keep app.token",["secret-keep","app.token"],null],[5,"surface.read \"sketch\"",null,null]],"error":null,"seal":"3893be14039d3936"}}],"timers":false,"clock":0,"error":null}"##,
    ),
    (
        "grants-unparsed",
        r#"{"ops":[{"op":"grants","lines":[],"error":"the app's grants did not parse: line 2: `crewHost` is not a secret name ([a-z0-9._-]{1,64})","set":{"version":1,"entries":[[1,"net.fetch https://crew.test",["fetch","https","crew.test",443],null],[2,"secret.keep crewHost",null,"`crewHost` is not a secret name ([a-z0-9._-]{1,64})"]],"error":"the app's grants did not parse: line 2: `crewHost` is not a secret name ([a-z0-9._-]{1,64})","seal":"b5982a07c9ad54f7"}}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "textflow",
        r#"{"ops":[{"op":"textflow","contexts":[]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "create",
        r#"{"ops":[{"op":"create","id":7,"tag":"a","props":{"href":"/x","data-exact-on":"click input"},"css":"color: red; content: \"\\\"","handlers":["click","input"]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "create-bare",
        r#"{"ops":[{"op":"create","id":4294967295,"tag":"div","props":{},"css":"","handlers":[]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "head",
        r#"{"ops":[{"op":"head","title":"Home \"Conduit\"","description":null,"image":"/i.png","canonical":null,"robots":"noindex","status":404}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "head-bare",
        r#"{"ops":[{"op":"head","title":null,"description":null,"image":null,"canonical":null,"robots":null,"status":null}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "adopt",
        r#"{"ops":[{"op":"adopt","adopted":true},{"op":"adopt","adopted":false}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "props",
        r#"{"ops":[{"op":"props","id":3,"set":{"value":"a\tb"},"clear":["checked","disabled"]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "props-bare",
        r#"{"ops":[{"op":"props","id":0,"set":{},"clear":[]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "style",
        r#"{"ops":[{"op":"style","id":12,"css":"width: 1.5px"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "children",
        r#"{"ops":[{"op":"children","id":1,"ids":[]},{"op":"children","id":2,"ids":[5,6,4294967295]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "animate",
        r#"{"ops":[{"op":"animate","id":8,"property":"opacity","delay":0,"duration":16.5,"values":[0,0.25,1000000000000000000000]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "animate-pairs",
        r#"{"ops":[{"op":"animate","id":9,"property":"translate","delay":12.25,"duration":300,"values":[[0.1,0.2],[-3.5,0.0000001]]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "animate-stop",
        r#"{"ops":[{"op":"animate","id":10,"property":"scale","delay":0,"duration":0,"values":[]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "spring",
        r#"{"ops":[{"op":"animate","id":9,"property":"translate","at":2500.5,"delay":0,"duration":300,"values":[[0.1,0.2],[0,0]]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "retire-motion",
        r#"{"ops":[{"op":"retire-motion","id":11,"property":"transform"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "timelines",
        r#"{"ops":[{"op":"timelines"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "surface",
        r#"{"ops":[{"op":"surface","id":31,"name":"chart","values":{"points":[1.5,-2],"label":"a\"b"}}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request",
        r#"{"ops":[{"op":"request","ticket":41,"target":"feed \"home\"","scope":"net.fetch https://api.example","method":"POST","url":"https://api.example/articles?tag=a b","headers":[["content-type","application/json"],["x","\"q\""]],"body":"eyJhIjoxfQ==","cache":"reload"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-independent",
        r#"{"ops":[{"op":"request","ticket":42,"target":"feed \"home\"","scope":null,"nativeHttp":"independent","maxResponseBytes":1048576,"method":"GET","url":"https://api.example/big","headers":[],"body":"","cache":"default"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-body-from",
        r#"{"ops":[{"op":"request","ticket":48,"target":"feed \"home\"","scope":null,"method":"POST","url":"https://api.example/blob","headers":[],"bodyFrom":"app:/tmp/a.jpg","body":"","cache":"default"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-continue",
        r#"{"ops":[{"op":"continue","ticket":43,"token":9007199254740993}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-storage",
        r#"{"ops":[{"op":"storage","ticket":44,"payload":"{\"op\":\"get\",\"key\":\"k\"}","scope":"store.local notes"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-capture",
        r#"{"ops":[{"op":"surfaceWork","ticket":45,"mode":"capture","name":"sketch","scope":"surface.carry sketch"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-restore",
        r#"{"ops":[{"op":"surfaceWork","ticket":46,"mode":"restore","name":"sketch","scope":null,"body":"AQID+g=="}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "request-mixed",
        r#"{"ops":[{"op":"surfaceWork","ticket":47,"mode":"capture","name":"s","scope":null,"refusal":"surface request combines multiple host-work kinds"}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "destroy",
        r#"{"ops":[{"op":"destroy","id":13}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "at",
        r#"{"ops":[{"op":"at","ms":16.5},{"op":"at","ms":0.30000000000000004},{"op":"at","ms":-0}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "roots",
        r#"{"ops":[{"op":"roots","ids":[]},{"op":"roots","ids":[1,2,30]}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "command",
        r#"{"ops":[{"op":"command","name":"copy","args":[1.5,null,true,"t",null,[3,null]],"source":7}],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "finish-timers",
        r#"{"ops":[{"op":"destroy","id":1}],"timer_due_ms":16,"accepted":true,"timers":true,"clock":1234.5,"error":"refused: \"x\""}"#,
    ),
    (
        "finish-empty",
        r#"{"ops":[],"timers":false,"clock":0,"error":null}"#,
    ),
    (
        "finish-fraction",
        r#"{"ops":[],"timer_due_ms":0.1,"timers":true,"clock":0.0000001,"error":null}"#,
    ),
];

#[test]
fn every_op_writes_the_json_it_always_wrote() {
    let cases = cases();
    assert_eq!(cases.len(), GOLDEN.len());
    for ((name, text), (golden_name, golden)) in cases.iter().zip(GOLDEN) {
        assert_eq!(name, golden_name);
        assert_eq!(text, golden, "{name}");
    }
}
