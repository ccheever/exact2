//! LLP 1098 §5: `metadata=MediaMetadata(…)` claims the media session on
//! `audio` and `video`, the six actions are the element's events with an
//! optional `MediaSessionActionDetails`, the offsets are checked, and the two
//! test steps parse and reach the driver.

use exact_kernel::{Kernel, PropId, PropValue};
use exact_plan::EventKind;
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A player component: `decls` after the state, `attrs` on its `tag`.
fn app(decls: &str, tag: &str, attrs: &str) -> String {
    format!("fn meta(t: string): MediaMetadata = MediaMetadata(title=t, artist=\"Show\", album=\"\", artwork=\"assets/art.png\")\ncomponent Player\n  state ep = \"Episode 1\"\n  state seek = 0\n  action skipBy(sign: number, d: MediaSessionActionDetails)\n    seek = seek + sign * d.seekOffset\n  action seekAt(d: MediaSessionActionDetails)\n    seek = d.seekTime\n  action next\n    ep = \"Episode 2\"\n  action skip(n: number)\n    seek = seek + n\n{decls}  view\n    {tag} \"assets/ep.mp3\" testId=\"player\" {attrs}\n")
}

fn props(src: &str) -> Vec<Option<String>> {
    let r = Runner::boot(
        contract::compile(src).unwrap_or_else(|e| panic!("{e}\n{src}")),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = r.kernel().find_by_test_id("player")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    [
        PropId::MediaTitle,
        PropId::MediaArtist,
        PropId::MediaAlbum,
        PropId::MediaArtwork,
        PropId::SeekbackwardOffset,
        PropId::SeekforwardOffset,
    ]
    .map(|p| match node.props.get(p) {
        Some(PropValue::Str(s)) => Some(s.to_string()),
        Some(PropValue::Float(f)) => Some(f.to_string()),
        other => other.map(|v| format!("{v:?}")),
    })
    .to_vec()
}

fn some(s: &str) -> Option<String> {
    Some(s.into())
}

#[test]
fn metadata_lowers_to_four_props_from_literals_state_and_a_fn() {
    for tag in ["audio", "video"] {
        let literal = app("", tag, "metadata=MediaMetadata(title=\"Episode 1\", artist=\"Show\", album=\"\", artwork=\"\") seekforwardOffset=30 seekforward=skipBy(1)");
        assert_eq!(
            props(&literal),
            [
                some("Episode 1"),
                some("Show"),
                some(""),
                some(""),
                None,
                some("30")
            ],
            "{tag}"
        );
        let state = app("", tag, "metadata=MediaMetadata(title=ep, artist=\"Show\", album=ep, artwork=\"https://example.com/a.png\")");
        assert_eq!(
            props(&state)[..4],
            [
                some("Episode 1"),
                some("Show"),
                some("Episode 1"),
                some("https://example.com/a.png")
            ],
            "{tag}"
        );
        let from_fn = app("", tag, "metadata=meta(ep) seekbackwardOffset=15");
        assert_eq!(
            props(&from_fn),
            [
                some("Episode 1"),
                some("Show"),
                some(""),
                some("assets/art.png"),
                some("15"),
                None
            ],
            "{tag}"
        );
        // A copy with one field replaced is a `MediaMetadata` too.
        let copied = app(
            "",
            tag,
            "metadata=MediaMetadata(meta(ep), album=\"Season 2\")",
        );
        assert_eq!(props(&copied)[2], some("Season 2"), "{tag}");
    }
}

#[test]
fn the_session_is_refused_where_it_does_not_belong() {
    let cases = [
        (
            "component A\n  view\n    box metadata=MediaMetadata(title=\"a\", artist=\"\", album=\"\", artwork=\"\")\n",
            "lower-attr-tag",
            "`metadata` belongs to `audio` or `video`",
        ),
        (
            "component A\n  view\n    audio \"assets/a.mp3\" metadata=MediaMetadata(title=\"a\", artist=\"\", album=\"\")\n",
            "type-record-missing",
            "`artwork` is missing",
        ),
        (
            "shape MediaMetadata\n  title: string\ncomponent A\n  view\n    text \"a\"\n",
            "type-duplicate-shape",
            "MediaMetadata",
        ),
        (
            "component A\n  action go\n    stopSounds()\n  view\n    audio \"assets/a.mp3\" seekforward=go\n",
            "lower-media-session",
            "`seekforward=` belongs to a media session: give this `audio` a `metadata=`",
        ),
        (
            "component A\n  view\n    video \"assets/a.mp4\" seekforwardOffset=30\n",
            "lower-media-session",
            "`seekforwardOffset=` belongs to a media session: give this `video` a `metadata=`",
        ),
        (
            "component A\n  action go\n    stopSounds()\n  view\n    box nexttrack=go\n",
            "lower-attr-tag",
            "`nexttrack` belongs to an `audio` or `video`'s media session, not `box`",
        ),
        (
            "component A\n  view\n    audio \"assets/a.mp3\" metadata=\"Episode\"\n",
            "lower-attr-type",
            "`metadata` takes a `MediaMetadata(title=…, artist=…, album=…, artwork=…)`, given `string`",
        ),
        (
            "fn MediaMetadata(t: string): string = t\ncomponent A\n  view\n    text \"a\"\n",
            "type-fn-shape-name",
            "`fn MediaMetadata` has a shape's name",
        ),
    ];
    for (src, id, message) in cases {
        let e = contract::compile(src).unwrap_err();
        assert_eq!(e.id, id, "{src}\n{e}");
        assert!(e.message.contains(message), "{src}\n{e}");
    }
    for offset in ["0", "-5"] {
        let src = app(
            "",
            "audio",
            &format!("metadata=meta(ep) seekbackwardOffset={offset}"),
        );
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(
            (e.id.as_str(), e.message.as_str()),
            (
                "lower-attr-value",
                "`seekbackwardOffset` is a number of seconds greater than 0"
            ),
            "{offset}"
        );
    }
}

#[test]
fn each_action_takes_the_record_or_leaves_it() {
    for event in [
        "seekbackward",
        "seekforward",
        "seekto",
        "previoustrack",
        "nexttrack",
        "stop",
    ] {
        for (decls, handler) in [
            ("  action bare\n    seek = 1\n", "bare"),
            ("  action rec(d: MediaSessionActionDetails)\n    seek = d.seekOffset + d.seekTime\n", "rec"),
            ("  action both(n: number, d: MediaSessionActionDetails)\n    seek = n + d.seekOffset\n", "both(2)"),
            ("  action inferred(d)\n    seek = d.seekOffset\n", "inferred"),
        ] {
            let src = app(decls, "audio", &format!("metadata=meta(ep) {event}={handler}"));
            let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{event}={handler}: {e}"));
            assert!(
                plan.handlers.iter().any(|h| h.event == EventKind::from_name(event).unwrap()),
                "{event}"
            );
        }
        // A missing argument, or one too many, is refused with what fits.
        let src = app("", "audio", &format!("metadata=meta(ep) {event}=skip"));
        let e = contract::compile(&src).unwrap_err();
        assert!(e.id.ends_with("arity"), "{event}: {e}");
        assert!(e.message.contains(event), "{event}: {e}");
        let src = app("", "audio", &format!("metadata=meta(ep) {event}=next(1)"));
        assert!(contract::compile(&src).is_err(), "{event}");
    }
}

#[test]
fn the_steps_parse_and_reach_the_driver() {
    let src = "test \"lock screen\"\n  tap \"audio\" mediasession \"seekto\" 600\n  tap \"audio\" mediasession \"seekforward\"\n  tap \"audio\" mediasession \"seekbackward\" 15\n  tap \"audio\" mediasession \"play\"\n  expect mediasession title == \"Episode 1\"\n  expect mediasession owner == none\n  expect mediasession has \"nexttrack\"\n  expect mediasession missing \"seekto\"\n";
    let tests = contract::tests(src).unwrap();
    let json = contract::tests_json(&tests);
    for want in [
        r#"{"op":"tap","target":"audio","form":"mediasession","action":"seekto","seconds":600,"line":2}"#,
        r#"{"op":"tap","target":"audio","form":"mediasession","action":"seekforward","line":3}"#,
        r#"{"op":"tap","target":"audio","form":"mediasession","action":"seekbackward","seconds":15,"line":4}"#,
        r#"{"op":"tap","target":"audio","form":"mediasession","action":"play","line":5}"#,
        r#"{"op":"expect-mediasession","field":"title","value":"Episode 1","line":6}"#,
        r#"{"op":"expect-mediasession","field":"owner","value":null,"line":7}"#,
        r#"{"op":"expect-mediasession","action":"nexttrack","present":true,"line":8}"#,
        r#"{"op":"expect-mediasession","action":"seekto","present":false,"line":9}"#,
    ] {
        assert!(json.contains(want), "{want}\n{json}");
    }
    for (bad, why) in [
        (
            "tap \"audio\" mediasession \"seekto\"",
            "takes the time to seek to",
        ),
        (
            "tap \"audio\" mediasession \"nexttrack\" 5",
            "takes no seconds",
        ),
        ("tap \"audio\" mediasession \"pause\" 1", "takes no seconds"),
        (
            "tap \"audio\" mediasession \"skipad\"",
            "is not a media session action",
        ),
        ("tap \"audio\" mediasession \"seekforward\" -3", "0 or more"),
        ("expect mediasession duration == \"1\"", "owner, title"),
        (
            "expect mediasession has \"rewind\"",
            "is not a media session action",
        ),
        ("expect mediasession title == none", "a string"),
    ] {
        let e = contract::tests(&format!("test \"t\"\n  {bad}\n")).unwrap_err();
        assert!(e.message.contains(why), "{bad}: {e}");
    }
}
