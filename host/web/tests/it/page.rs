//! The page around a document (LLP 1048.000 D3, D6, D7): its head, its
//! checkpoint, and the locations a build renders.

use exact_runner::{DataError, DataSource, Value};
use exact_web::document::{build_locations, checkpoint, digest, read_checkpoint, route_at, Site};
use exact_web::Host;

#[derive(Clone, Default)]
struct Says(&'static str);

impl DataSource for Says {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::str(self.0))
    }
}

fn host(src: &str, data: Says, launch: &str) -> Host<Says> {
    let plan = contract::bake(contract::compile(src).unwrap(), data.clone()).unwrap();
    Host::boot(&plan.encode(), data, Default::default(), launch)
        .unwrap()
        .0
}

const SITE: Site<'static> = Site {
    name: "Site & Co",
    origin: Some("https://example.com/"),
};

#[test]
fn the_head_carries_the_active_head_with_urls_a_crawler_can_follow() {
    let src = r#"
component A
  resource said = say() as shape string
  view
    column
      head title=said description="A <b>bold</b> \"claim\"" image="/assets/card.png" canonical="/post/7" robots="index, follow"
      text "a"
"#;
    let host = host(src, Says("Post & <Title>"), "/post/7");
    let head = host
        .document()
        .unwrap()
        .page_head(host.runner().plan(), &SITE, "/post/7")
        .unwrap();
    for want in [
        "<title>Post &amp; &lt;Title&gt;</title>",
        r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#,
        r#"<meta name="description" content="A &lt;b&gt;bold&lt;/b&gt; &quot;claim&quot;">"#,
        r#"<meta name="robots" content="index, follow">"#,
        r#"<link rel="canonical" href="https://example.com/post/7">"#,
        r#"<meta property="og:site_name" content="Site &amp; Co">"#,
        r#"<meta property="og:url" content="https://example.com/post/7">"#,
        r#"<meta property="og:image" content="https://example.com/assets/card.png">"#,
        r#"<meta name="twitter:card" content="summary_large_image">"#,
    ] {
        assert!(head.contains(want), "{want}\n{head}");
    }
    assert!(!head.contains("preload"), "{head}");
}

#[test]
fn without_a_head_the_title_is_the_apps_and_nothing_else_is_claimed() {
    let plain = host(
        "component A\n  view\n    column viewport-fit=\"cover\" interactive-widget=\"resizes-content\"\n      text \"a\"\n",
        Says(""),
        "/",
    );
    let doc = plain.document().unwrap();
    let no_origin = Site {
        name: "App",
        origin: None,
    };
    let head = doc
        .page_head(plain.runner().plan(), &no_origin, "/")
        .unwrap();
    assert!(head.starts_with("<title>App</title>"), "{head}");
    assert!(head.contains(
        r#"content="width=device-width, initial-scale=1, viewport-fit=cover, interactive-widget=resizes-content""#
    ));
    for absent in ["description", "canonical", "og:url", "og:image", "robots"] {
        assert!(!head.contains(absent), "{absent}: {head}");
    }
    assert!(head.contains(r#"<meta name="twitter:card" content="summary">"#));
    // A script-bearing canonical or image is dropped, never written.
    let host2 = host(
        "component A\n  view\n    column\n      head canonical=\"javascript:alert(1)\" image=\"javascript:alert(2)\"\n      text \"a\"\n",
        Says(""),
        "/",
    );
    let head = host2
        .document()
        .unwrap()
        .page_head(host2.runner().plan(), &SITE, "/")
        .unwrap();
    assert!(!head.contains("javascript"), "{head}");
}

#[test]
fn the_checkpoint_never_closes_its_script_or_opens_a_comment() {
    let host = host("component A\n  view\n    text \"a\"\n", Says(""), "/");
    let hostile = "/x</script><!--]]>&";
    let json = checkpoint(host.runner(), hostile);
    let (lt, gt, amp) = ("\\u003c", "\\u003e", "\\u0026");
    assert_eq!(
        json,
        format!(
            "{{\"location\":\"/x{lt}/script{gt}{lt}!--]]{gt}{amp}\",\"time\":0,\"logic\":null,\"answers\":\"BgAAAAA=\",\"pending\":[]}}"
        )
    );
    for bad in ["<", ">", "&"] {
        assert!(!json.contains(bad), "{json}");
    }
}

#[test]
fn a_checkpoint_reads_back_as_the_runner_wrote_it() {
    // Answers carry user text; the page carries them as base64.
    let host = host(
        "component A\n  resource said = say() as shape string\n  view\n    text said\n",
        Says("</script><!-- & \u{1F4AC}"),
        "/said?q=1",
    );
    let json = checkpoint(host.runner(), "/said?q=1");
    assert!(
        !json.contains('<') && !json.contains('>') && !json.contains('&'),
        "{json}"
    );
    let read = read_checkpoint(&json).unwrap();
    assert_eq!(read, host.runner().document_checkpoint("/said?q=1"));
    assert_eq!(read.answers[0].3, Value::str("</script><!-- & \u{1F4AC}"));
    for broken in [
        "",
        "{}x",
        "{\"time\":\"0\"}",
        "{\"answers\":\"!!\"}",
        "{\"extra\":1}",
    ] {
        assert!(read_checkpoint(broken).is_err(), "{broken}");
    }
}

#[test]
fn a_runtime_adopts_the_document_it_would_have_rendered() {
    let plan = caltrain::build().unwrap().encode();
    let boot = |launch: &str| {
        Host::boot(&plan, caltrain_data::Caltrain, Default::default(), launch)
            .unwrap()
            .0
    };
    // What a render at `/` writes into the page.
    let rendered = boot("/");
    let page = checkpoint(rendered.runner(), "/");
    let root = rendered.document().unwrap().root;
    let written = digest(rendered.runner().plan(), "/", &page, &root);
    let open = |launch: &str, page: &str, digest: &str| {
        let (host, batch) = Host::boot_checkpoint(
            &plan,
            caltrain_data::Caltrain,
            page,
            digest,
            Vec::new(),
            None,
            Default::default(),
            launch,
        )
        .unwrap();
        (host, batch)
    };
    let adopted = |batch: &str| batch.contains("{\"op\":\"adopt\",\"adopted\":true}");
    let (host, batch) = open("/", &page, &written);
    assert!(adopted(&batch), "{}", &batch[..200.min(batch.len())]);
    assert!(host
        .runner()
        .journal()
        .any(|l| l.contains("checkpoint: 7 of 7 answers taken")));
    // A query the route table doesn't read, or another spelling of the
    // same path, is the same page: the runtime boots at the page's path,
    // keeping the browser's query, and adopts.
    let (host, batch) = open("//?utm=x", &page, &written);
    assert!(adopted(&batch));
    assert!(batch.contains("\"url\":\"/?utm=x\""), "{batch}");
    assert_eq!(host.location(), "/?utm=x");
    // Another path isn't this page's (a static host's 404.html, its
    // fallback to `/`), even where the tree would match: render fresh.
    let (host, batch) = open("/elsewhere?utm=x", &page, &written);
    assert!(!batch.contains("\"op\":\"adopt\""));
    assert!(host
        .runner()
        .journal()
        .any(|l| l.contains("document: rendered at /, not /elsewhere?utm=x; rendering fresh")));
    assert!(!adopted(&open("/", &page, &"0".repeat(64)).1));
    let other = checkpoint(rendered.runner(), "/other");
    let digest_other = digest(rendered.runner().plan(), "/other", &other, "<p>not it</p>");
    assert!(!adopted(&open("/other", &other, &digest_other).1));
    // A checkpoint that doesn't read boots as a page without one.
    let (host, batch) = open("/", "{not json", &written);
    assert!(!batch.contains("\"op\":\"adopt\""));
    assert!(host
        .runner()
        .journal()
        .any(|l| l.contains("document: the checkpoint doesn't read")));
}

#[test]
fn a_build_renders_the_routes_that_ask_and_the_not_found_document() {
    let plan = contract::compile(
        "routes nav\n  tab home \"/\" render=build\n    about \"/about\" render=build\n    post \"/post/:post\" render=request\n  tab client \"/client\"\n  notfound render=build\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    assert_eq!(
        build_locations(&plan).unwrap(),
        vec![
            ("/".to_owned(), false),
            ("/about".to_owned(), false),
            ("/404.html".to_owned(), true)
        ]
    );
    // Undeclared routes render nothing at build.
    let plain = contract::compile(
        "routes nav\n  tab home \"/\"\n  notfound\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    assert!(build_locations(&plain).unwrap().is_empty());
    // The not-found document's location must be one no pattern matches.
    let clash = contract::compile("routes nav\n  tab home \"/\" render=build\n  page \"/404.html\"\n  notfound render=build\ncomponent A\n  view\n    text \"a\"\n").unwrap();
    assert!(build_locations(&clash)
        .unwrap_err()
        .contains("route `page` matches"));
}

#[test]
fn the_plans_fonts_are_declared_for_a_reader_without_javascript() {
    // TypeTour's directory, for its font files.
    let app =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/typetour/app.contract");
    let src = "font \"Exciting Lilt\"\n  700 = \"assets/ExcitingLilt-Bold.ttf\"\ncomponent A\n  view\n    text \"a\" font-family=\"Exciting Lilt\" font-weight=700\n";
    let plan = contract::compile_path_source(&app, src).unwrap();
    let host = Host::boot(&plan.encode(), Says(""), Default::default(), "/")
        .unwrap()
        .0;
    let doc = host.document().unwrap();
    let head = doc.page_head(host.runner().plan(), &SITE, "/").unwrap();
    let family = head
        .split("@font-face{font-family:\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    assert!(family.starts_with("ExactPlanStack"), "{head}");
    assert!(
        head.contains(&format!(
            r#"<style>@font-face{{font-family:"{family}";src:url("assets/ExcitingLilt-Bold.ttf");font-weight:700;font-style:normal}}</style>"#
        )),
        "{head}"
    );
    assert!(
        head.contains(r#"<link rel="preload" href="assets/ExcitingLilt-Bold.ttf" as="font" type="font/ttf" crossorigin>"#),
        "{head}"
    );
    // The document's text asks for the same family name.
    assert!(
        doc.root
            .contains(&format!("font-family:&quot;{family}&quot;")),
        "{}",
        doc.root
    );
}

#[test]
fn a_location_resolves_to_its_route_or_the_not_found_route() {
    let plan = contract::compile(
        "routes nav\n  tab home \"/\" render=build\n    post \"/post/:post\" render=cached\n  notfound\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let name = |at: &str| route_at(&plan, at).map(|r| plan.str(r.name).to_string());
    assert_eq!(name("/").as_deref(), Some("home"));
    assert_eq!(name("/post/7?x=1").as_deref(), Some("post"));
    assert!(route_at(&plan, "/nowhere").unwrap().notfound);
    let plain = contract::compile("component A\n  view\n    text \"a\"\n").unwrap();
    assert!(route_at(&plain, "/").is_none());
}
