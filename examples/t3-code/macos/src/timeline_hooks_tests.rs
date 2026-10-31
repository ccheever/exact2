//! The compiled plan asks for its snapshot with
//! `exactViewport().displayMode`, so the title rows' inset and the sidebar minimum
//! (presentation.ts, sidebar-min-width.ts) follow the window into full screen and
//! back, with no native full-screen fact (T3FullScreen.swift is gone). The data
//! module is a stand-in that records each `snapshot` request and leaves it pending.

use super::composer_accessibility_tests::measure;
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_runner::{Answer, DataError, DataSource, Preferences, Request, Store, Value};

#[derive(Default)]
struct Snapshots {
    asked: Vec<Vec<Value>>,
}

impl DataSource for Snapshots {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source == "snapshot" {
            self.asked.push(args.to_vec());
        }
        Ok(Answer::Later(Request::get("https://fixture.invalid/")))
    }
}

/// The display mode (the fourth argument) of the newest snapshot request.
fn mode(host: &mut exact_apple::Host<Snapshots>) -> Value {
    let asked = &host.runner_mut().data().asked;
    asked.last().expect("a snapshot request")[3].clone()
}

#[test]
fn the_snapshot_request_follows_the_viewport_display_mode() {
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) = exact_apple::Host::boot(
        PLAN,
        Snapshots::default(),
        Box::new(measurer),
        1280.0,
        840.0,
    )
    .unwrap();
    host.runner_mut()
        .set_time(1_791_446_400_000.0, 0.0)
        .unwrap();
    host.runner_mut().advance(1250.0).unwrap();
    assert_eq!(mode(&mut host), Value::str("standalone"));
    // LLP 1039: display mode 2 is `fullscreen` (the host's NSWindow full-screen state).
    host.set_preferences(Preferences::from_bits(2 << 11));
    assert_eq!(mode(&mut host), Value::str("fullscreen"));
    host.set_preferences(Preferences::from_bits(0));
    assert_eq!(mode(&mut host), Value::str("standalone"));
}

#[test]
fn declared_message_records_reach_markdown_with_find_anchors() {
    use exact_js::{from_json, to_json, Shape};
    use exact_plan::Plan;

    fn empty(shape: &Shape) -> Value {
        match shape {
            Shape::Number => Value::Number(0.0),
            Shape::Bool => Value::Bool(false),
            Shape::String => Value::str(""),
            Shape::Unit => Value::Unit,
            Shape::Option(_) => Value::NONE,
            Shape::List(_) => Value::list(Vec::new()),
            Shape::Record(fields) => {
                Value::record(fields.iter().map(|(_, shape)| empty(shape)).collect())
            }
        }
    }
    fn marked(value: &Value) -> Vec<(String, u8)> {
        match value {
            // A FindPart: its id, text, mark and offset.
            Value::Record(fields)
                if fields.len() == 4
                    && fields[1].as_str().is_some()
                    && fields[2].as_number().is_some()
                    && fields[3].as_number().is_some() =>
            {
                let mark = fields[2].as_number().unwrap() as u8;
                if mark == 0 {
                    Vec::new()
                } else {
                    vec![(fields[1].text().to_owned(), mark)]
                }
            }
            Value::Record(fields) | Value::List(fields) => fields.iter().flat_map(marked).collect(),
            _ => Vec::new(),
        }
    }

    let plan = Plan::decode(PLAN).unwrap();
    let data = plan
        .resources
        .iter()
        .find(|row| plan.str(row.name) == "data")
        .unwrap();
    let messages = plan
        .type_(data.ty)
        .fields
        .iter()
        .find(|field| plan.str(plan.field(*field).name) == "messages")
        .unwrap();
    let message = plan.type_(plan.field(messages).ty).elem.unwrap();
    let shape = Shape::from_plan(&plan, message).unwrap();
    let Shape::Record(fields) = &shape else {
        panic!("Message is a record")
    };
    let Shape::List(secret) = &fields.iter().find(|(name, _)| name == "secret").unwrap().1 else {
        panic!("Message.secret is a list")
    };

    for (role, title, source, expected) in [
        ("user", "You", "need[Thread](t3-context://v1/thread/thread_t1)le needle\n\nneedle[Thread](t3-context://v1/thread/thread_t1)needle", 3),
        ("assistant", "Assistant", "**needle**", 1),
        ("plan", "needle plan", "body needle", 2),
    ] {
        let mut json = to_json(&empty(&shape), &shape).unwrap();
        json["id"] = "declared-message".into();
        json["kind"] = role.into();
        json["title"] = title.into();
        json["body"] = source.into();
        json["secret"] = vec![to_json(&empty(secret), secret).unwrap()].into();
        let message = from_json(&json, &shape).unwrap();
        assert!(message.conforms(&plan, plan.type_(plan.field(messages).ty).elem.unwrap()));
        for occurrence in 0..expected {
            let result = t3_code_data::Markdown.query("renderMarkdown", &[
                Value::list(vec![message.clone()]), Value::list(Vec::new()),
                Value::str("needle"), Value::str("declared-message"),
                Value::Number(occurrence as f64), Value::list(Vec::new()),
            ]).unwrap();
            let Value::Record(result) = result else { panic!("TranscriptMarkdown is a record") };
            let Value::List(documents) = &result[0] else { panic!("documents is a list") };
            assert_eq!(documents.len(), 1, "{role} disappeared at the Message/Rust boundary");
            let Value::Record(document) = &documents[0] else { panic!("document is a record") };
            assert_eq!(document[0], Value::str("declared-message"));
            let found = marked(&documents[0]);
            assert_eq!(found.len(), expected, "{role}");
            for (index, (text, mark)) in found.iter().enumerate() {
                assert_eq!(text, "needle", "{role}");
                // Plan titles precede the body in search, but follow it in ChatDocument.
                let active = if role == "plan" { expected - 1 - occurrence } else { occurrence };
                assert_eq!(*mark, if index == active { 3 } else { 1 }, "{role}");
            }
        }
    }
}
