//! An async function resumes under the answer it began in (LLP 1133.002
//! D14): three answers await one promise the first made, then fetch their
//! own rows, and every request after the await arrives under its own
//! answer. Before the bundle lowered `async` through the context helper
//! (js/bake/src/typescript.mjs `asyncContextHelper`), a continuation ran
//! under whichever answer's fetch resolved the shared promise.
#![cfg(exact_js_engine)]
use super::background::drive;
use exact_js::Module;
use exact_plan::Value;
use exact_runner::{DataSource, Outcome, Response, Store};
use std::sync::Mutex;

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/context.hbc"));
const GRANTS: &str = "net.fetch https://context.test\n";

fn plan() -> exact_plan::Plan {
    contract::compile(
        r#"
shape Result
  text: string
component App
  mutation output as shape Result
  action go(name: string)
    send output = work(name)
  view
    text "context"
"#,
    )
    .unwrap()
}

#[test]
fn each_answer_fetches_as_itself_after_awaiting_another_answers_promise() {
    let mut m = Module::new(HBC.to_vec(), "test.context", GRANTS);
    m.set_budget_ms(f64::INFINITY);
    m.bind(&plan());
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    let names = ["a", "b", "c"];
    let answers = names
        .iter()
        .map(|n| {
            let a = vec![Value::str(n)];
            let answer = m.answer(&mut s, "work", &a).unwrap();
            (a, answer)
        })
        .collect();
    let seen = Mutex::new(Vec::new());
    let mut done = drive(&mut m, &mut s, answers, |a, request| {
        let asker = a[0].as_str().unwrap().to_owned();
        let path = request.url.trim_start_matches("https://context.test/");
        seen.lock().unwrap().push((asker, path.to_owned()));
        Outcome::Response(Response {
            status: 200,
            headers: Vec::new(),
            body: path.as_bytes().to_vec(),
        })
    });
    done.sort();
    assert_eq!(
        done,
        [
            "shared a/one a/two a/helper",
            "shared b/one b/two b/helper",
            "shared c/one c/two c/helper",
        ]
    );
    for (asker, path) in seen.into_inner().unwrap() {
        if path != "shared" {
            assert!(
                path.starts_with(&format!("{asker}/")),
                "{path} was asked under answer {asker}"
            );
        }
    }
}
