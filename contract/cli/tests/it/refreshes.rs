//! LLP 1054.000.000: a mutation's declared `refreshes`, and a re-ask that
//! keeps the identical request already in flight.

use exact_kernel::{Kernel, PropId};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Runner, Value};

const SRC: &str = r#"
shape Msg
  text: string

component App
  state page = 0
  state token = "a"
  mutation dm as shape Msg refreshes chats
  mutation like as shape Msg
  mutation fav as shape Msg refreshes feed
  mutation star as shape Msg refreshes feed
  resource chats = chats() as shape list<Msg>
  resource feed = feed(page, token) as shape list<Msg>

  action say(t: string)
    send dm = say(t)
  action tap
    send like = like()
  action favor
    send fav = like()
  action starNow
    send star = star()
  action next
    page = page + 1
  action relogin
    token = token == "a" ? "b" : "a"
  action reload
    refresh feed

  view
    column
      text `${length(chats)}` testId="chats"
      text `${length(feed)}` testId="feed"
"#;

/// A chat whose sent messages show at once (the source's own overlay) and a
/// feed whose request names neither its page nor anything but the token.
#[derive(Default)]
struct Chat {
    live: bool,
    pending: Vec<String>,
    sent: Vec<String>,
    chat_asks: usize,
    feed_parsed_with: Vec<Vec<Value>>,
}

fn msgs(texts: impl Iterator<Item = String>) -> Value {
    Value::list(texts.map(|t| Value::record(vec![Value::str(&t)])).collect())
}

impl DataSource for Chat {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "chats" => {
                self.chat_asks += 1;
                Ok(msgs(self.sent.iter().chain(&self.pending).cloned()))
            }
            "feed" => Ok(Value::list(vec![])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "say" => {
                self.pending
                    .push(args[0].as_str().unwrap_or("").to_string());
                Ok(Answer::Later(Request::post_json(
                    "https://chat.test/say",
                    "{}",
                )))
            }
            // Answered at once: the write has happened when it returns.
            "star" => Ok(Answer::Now(Value::record(vec![Value::str("")]))),
            "like" => Ok(Answer::Later(Request::post_json(
                "https://chat.test/like",
                "{}",
            ))),
            "feed" if self.live => Ok(Answer::Later(
                Request::get("https://chat.test/feed")
                    .header("authorization", args[1].as_str().unwrap_or("")),
            )),
            _ => self.query(source, args).map(Answer::Now),
        }
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        match source {
            "say" => {
                let t = self.pending.remove(0);
                self.sent.push(t.clone());
                Ok(Answer::Now(Value::record(vec![Value::str(&t)])))
            }
            "like" => Ok(Answer::Now(Value::record(vec![Value::str("")]))),
            "feed" => {
                self.feed_parsed_with.push(args.to_vec());
                Ok(Answer::Now(msgs(std::iter::once("post".to_string()))))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    })
}

fn boot() -> Runner<Chat> {
    let baked = contract::bake(contract::compile(SRC).unwrap(), Chat::default()).unwrap();
    let data = Chat {
        live: true,
        ..Chat::default()
    };
    Runner::boot(
        baked,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn text(r: &Runner<Chat>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

fn journal_has(r: &Runner<Chat>, needle: &str) -> bool {
    r.journal().any(|l| l.contains(needle))
}

#[test]
fn a_send_refreshes_what_its_mutation_declares_when_sent_and_when_answered() {
    let mut r = boot();
    let asks = r.data().chat_asks;
    r.act("say", vec![Value::str("hi")]).unwrap();
    // Asked again in the sending commit: the source's overlay shows.
    assert_eq!(r.data().chat_asks, asks + 1);
    assert_eq!(text(&r, "chats"), "1");
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    // And again when the reply landed, in that commit.
    assert_eq!(r.data().chat_asks, asks + 2);
    assert_eq!(text(&r, "chats"), "1");
    // A mutation that declares nothing refreshes nothing.
    r.act("tap", vec![]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    assert_eq!(r.data().chat_asks, asks + 2);
}

#[test]
fn an_argument_change_keeps_the_identical_request_in_flight() {
    let mut r = boot();
    r.act("next", vec![]).unwrap();
    let first = r.take_requests();
    assert_eq!(first.len(), 1);
    // The page moved again during the load; the request is the same.
    r.act("next", vec![]).unwrap();
    assert!(r.take_requests().is_empty(), "nothing new is sent");
    assert!(journal_has(
        &r,
        &format!("keep request {} (feed)", first[0].ticket)
    ));
    assert!(r.holds(first[0].ticket));
    // The reply is parsed with the newest arguments, and taken.
    r.fulfill(first[0].ticket, ok()).unwrap();
    assert_eq!(
        r.data().feed_parsed_with,
        vec![vec![Value::Number(2.0), Value::str("a")]]
    );
    assert_eq!(text(&r, "feed"), "1");
    // Settled against those arguments: nothing is asked again.
    r.act("tap", vec![]).unwrap();
    assert!(r.take_requests().iter().all(|q| q.target != "feed"));
}

#[test]
fn a_forced_re_ask_or_a_different_request_is_never_kept() {
    let mut r = boot();
    r.act("next", vec![]).unwrap();
    let first = r.take_requests()[0].ticket;
    // `refresh` wants an answer from after it: a new request.
    r.act("reload", vec![]).unwrap();
    let second = r.take_requests();
    assert_eq!(second.len(), 1);
    assert!(second[0].forced && !r.holds(first));
    // A changed header is a different question: a new request.
    r.act("relogin", vec![]).unwrap();
    let third = r.take_requests();
    assert_eq!(third.len(), 1);
    assert!(!r.holds(second[0].ticket));
    // Sends are effects: two equal sends are two requests.
    r.act("tap", vec![]).unwrap();
    r.act("tap", vec![]).unwrap();
    assert_eq!(r.take_requests().len(), 2);
}

#[test]
fn refreshes_names_this_component_s_resources_once_each() {
    let src = |names: &str| {
        SRC.replace(
            "mutation dm as shape Msg refreshes chats",
            &format!("mutation dm as shape Msg refreshes {names}"),
        )
    };
    assert_eq!(
        contract::compile(&src("page")).unwrap_err().id,
        "type-refreshes-not-resource"
    );
    assert_eq!(
        contract::compile(&src("chats, chats")).unwrap_err().id,
        "type-refreshes-duplicate"
    );
    let plan = contract::compile(&src("chats, feed")).unwrap();
    assert_eq!(
        plan.mutation_refreshes.len(),
        4,
        "chats and feed, and fav's and star's feed"
    );
}

#[test]
fn a_network_resource_is_asked_the_host_only_when_the_reply_lands() {
    let mut r = boot();
    r.act("next", vec![]).unwrap();
    let loading = r.take_requests()[0].ticket;
    // At the send the source is asked again, but a request it hands back
    // would read the server before the write: nothing is sent, and the load
    // in flight stays.
    r.act("favor", vec![]).unwrap();
    let sent = r.take_requests();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].target, "fav");
    assert!(r.holds(loading));
    // The reply lands: the feed is fetched again, forced.
    r.fulfill(sent[0].ticket, ok()).unwrap();
    let after = r.take_requests();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].target, "feed");
    assert!(after[0].forced && !r.holds(loading));
}

/// A mutation answered at once has landed in the sending commit: what it
/// declares is forced then, as a reply's landing forces it. A re-read there
/// dropped the feed's request and no reply came to ask again, so the feed
/// kept its old value (found by the x2apps data6 lane on the web build and
/// macOS alike).
#[test]
fn a_mutation_answered_at_once_forces_its_network_resource_in_that_commit() {
    let mut r = boot();
    r.act("next", vec![]).unwrap();
    let loading = r.take_requests()[0].ticket;
    r.act("starNow", vec![]).unwrap();
    let sent = r.take_requests();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].target, "feed");
    assert!(sent[0].forced && !r.holds(loading));
}
