//! `@exact/payments` under its declared substitute (LLP 1133.034 §9), the
//! package's own `exact.json`: a Contract send holds one `payment` ticket
//! for the wallet's sheet, which asks the app's handlers (a Rust stand-in
//! for its data module's `tools`) through `invoke` while it is up; its
//! authorization carries a synthetic payment that no ticket, journal line
//! or `state.extensions` shows; a let-go send never cancels a running
//! final handler; budgets run on the session's clock; and D6's phases
//! differ by target as each wallet's do.

use exact_runner::{
    Answer, CallOrigin, DataError, DataSource, Dispatch, HoldAnswer, Outcome, Reply, Request,
    Response, Runner, Store, Value, Work,
};
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

const APPLE: &str = r#"PaymentMethodData(supportedMethods="https://apple.com/apple-pay", data=some("{\"version\":3,\"merchantIdentifier\":\"MERCHANT\",\"merchantCapabilities\":[\"supports3DS\"],\"supportedNetworks\":[\"visa\"],\"countryCode\":\"US\"}"))"#;

/// The app: `buy` sends the sheet; `leave`, as leaving its screen does,
/// lets the send go by forgetting its reply.
fn app(merchant: &str, google: &str, currency: &str, handlers: &str) -> String {
    let apple = APPLE.replace("MERCHANT", merchant);
    format!(
        r#"use showPaymentRequest, PaymentResult, PaymentMethodData, PaymentDetailsInit, PaymentItem, PaymentCurrencyAmount, PaymentShippingOption, PaymentOptions from "@exact/payments"

component App
  mutation ordered as shape PaymentResult
  derive methods = [{apple}, PaymentMethodData(supportedMethods="https://google.com/pay", data=some("{google}"))]
  derive total = PaymentItem(label="Photo Editor Prints", amount=PaymentCurrencyAmount(currency="{currency}", value="8.00"), pending=none)
  derive standard = PaymentShippingOption(id="standard", label="Standard", amount=PaymentCurrencyAmount(currency="USD", value="0.00"), selected=some(true))
  derive details = PaymentDetailsInit(id=some("c1"), total=total, displayItems=none, shippingOptions=some([standard]), modifiers=none)
  derive asked = PaymentOptions(requestPayerName=none, requestPayerEmail=some(true), requestPayerPhone=none, requestShipping=some(true), requestBillingAddress=none, shippingType=none)
  action buy
    send ordered = showPaymentRequest(methodData=methods, details=details, options=asked{handlers})
  action leave
    ordered = none
  view
    column
      text "prints"
"#
    )
}

const GOOGLE: &str = r#"{\"apiVersion\":2,\"apiVersionMinor\":0,\"allowedPaymentMethods\":[{\"type\":\"CARD\",\"parameters\":{\"allowedAuthMethods\":[\"PAN_ONLY\"],\"allowedCardNetworks\":[\"VISA\"]},\"tokenizationSpecification\":{\"type\":\"PAYMENT_GATEWAY\",\"parameters\":{\"gateway\":\"example\",\"gatewayMerchantId\":\"exampleGatewayMerchantId\"}}}],\"merchantInfo\":{\"merchantName\":\"Photo Editor Prints\"}}"#;
const HANDLERS: &str = r#", onmerchantvalidation="validateMerchant", onshippingaddresschange="quoteShipping", onshippingoptionchange="quoteShipping", onpaymentauthorized="placeOrder""#;
const MERCHANT: &str = "merchant.com.exact.photoeditor";
const GRANTS: &str = "payment.apple-pay merchant.com.exact.photoeditor\npayment.google-pay\nnet.fetch http://prints.test\n";

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-payments-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/payments");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/payments");
        for file in [
            "package.json",
            "exact.json",
            "apple/exact.json",
            "android/exact.json",
            "web/exact.json",
        ] {
            std::fs::create_dir_all(package.join(file).parent().unwrap()).unwrap();
            std::fs::copy(source.join(file), package.join(file)).unwrap();
        }
        std::fs::write(path.join("app.contract"), contract).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The data module's `tools`, as the photo editor's would answer: each
/// call's argument is kept, and `placeOrder` answers as `mode` says
/// (`success`, `fail`, `retry`, or `hold`: a request to the print lab
/// that the test answers, or never does).
#[derive(Clone)]
struct Tools {
    calls: Arc<Mutex<Vec<(String, Json)>>>,
    mode: Arc<Mutex<&'static str>>,
    grants: &'static str,
}
impl DataSource for Tools {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let arg: Json = serde_json::from_str(args[0].as_str().unwrap()).unwrap();
        self.calls
            .lock()
            .unwrap()
            .push((source.to_owned(), arg.clone()));
        let answer = match source {
            "tool:validateMerchant" => json!({ "merchantSession": { "exactSubstitute": true } }),
            "tool:quoteShipping" if arg["shippingAddress"]["country"] == "ZZ" => {
                json!({ "error": "We don't ship there.", "shippingAddressErrors": { "country": "We ship to US." } })
            }
            "tool:quoteShipping" => json!({
                "total": { "label": "Photo Editor Prints", "amount": { "currency": "USD", "value": "13.00" } },
                "shippingOptions": [
                    { "id": "standard", "label": "Standard", "amount": { "currency": "USD", "value": "5.00" }, "selected": arg["shippingOption"] != "express" },
                    { "id": "express", "label": "Express", "amount": { "currency": "USD", "value": "15.00" }, "selected": arg["shippingOption"] == "express" },
                ],
            }),
            "tool:placeOrder" => match *self.mode.lock().unwrap() {
                "hold" => return Ok(Answer::Later(Request::get("http://prints.test/pay"))),
                "fail" => json!({ "result": "fail" }),
                "retry" => {
                    json!({ "retry": { "shippingAddress": { "postalCode": "Check the ZIP code." } } })
                }
                _ => json!({ "result": "success", "data": { "orderId": "PR-1" } }),
            },
            _ => return Err(DataError::UnknownSource(source.into())),
        };
        Ok(Answer::Now(Value::from(answer.to_string())))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(Response { body, .. }) = outcome else {
            return Err(DataError::Unavailable("no reply".into()));
        };
        Ok(Answer::Now(Value::from(String::from_utf8(body).unwrap())))
    }
    fn grants(&self) -> &str {
        self.grants
    }
}

/// The host: the runner, and the print lab's requests, which it never answers itself.
struct Host {
    r: Runner<Tools>,
    tools: Tools,
    lab: Vec<u64>,
    /// Held sends' executor tokens, and their requests' tickets.
    parked: Vec<(u64, u64)>,
    _app: App,
}

impl Host {
    fn boot(name: &str, target: &str, contract: &str, grants: &'static str) -> Host {
        let app = App::new(name, contract);
        let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let tools = Tools {
            calls: Default::default(),
            mode: Arc::new(Mutex::new("success")),
            grants,
        };
        let mut r = Runner::boot(
            plan,
            tools.clone(),
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.set_module_target(target);
        let mut host = Host {
            r,
            tools,
            lab: Vec::new(),
            parked: Vec::new(),
            _app: app,
        };
        host.pump();
        host
    }

    fn photo_editor(name: &str, target: &str) -> Host {
        Host::boot(
            name,
            target,
            &app(MERCHANT, GOOGLE, "USD", HANDLERS),
            GRANTS,
        )
    }

    /// Each request run (a held send's parked until its work is released),
    /// each outcome fulfilled, until nothing is left to do.
    fn pump(&mut self) {
        for _ in 0..16 {
            let outs = self.r.take_requests();
            let released = self.r.release_work();
            let mut moved =
                !outs.is_empty() | !released.is_empty() | !self.r.apply_announced().0.is_empty();
            for out in outs {
                let dispatch = match out.request.continuation {
                    Some(token) => {
                        self.parked.push((token, out.ticket));
                        self.r.dispatch_work(token)
                    }
                    None if out.request.is_native() => self.r.native_work(&out.request),
                    None => {
                        self.lab.push(out.ticket);
                        continue;
                    }
                };
                moved |= self.run(out.ticket, dispatch);
            }
            for (token, dispatch) in released {
                let ticket = (self.parked.iter())
                    .find(|(t, _)| *t == token)
                    .expect("parked")
                    .1;
                moved |= self.run(ticket, dispatch);
            }
            if !moved {
                return;
            }
        }
    }

    /// `dispatch`'s outcome for `ticket`, when it has one now.
    fn run(&mut self, ticket: u64, dispatch: Dispatch) -> bool {
        let (tx, rx) = mpsc::channel();
        match dispatch {
            Dispatch::Run(Work::Now(work)) => tx.send(work()).unwrap(),
            Dispatch::Run(Work::Later(work)) => work(Reply::new(move |o| tx.send(o).unwrap())),
            _ => return false,
        }
        let Ok(outcome) = rx.try_recv() else {
            return false;
        };
        let _ = self.r.fulfill(ticket, outcome);
        true
    }

    fn act(&mut self, action: &str) {
        self.r.act(action, vec![]).unwrap();
        self.pump();
    }

    /// The `payment` ticket held, and its args.
    fn sheet(&self) -> Option<(u64, Json)> {
        (self.r.device_holds().iter())
            .find(|h| h.capability == "payment")
            .map(|h| (h.ticket, serde_json::from_str(&h.args).unwrap()))
    }

    fn answer(&mut self, answer: HoldAnswer) -> Result<(), String> {
        let (ticket, _) = self.sheet().expect("a sheet");
        self.r.answer_hold(ticket, &answer).map(drop)?;
        self.pump();
        Ok(())
    }

    fn tap(&mut self, choice: &str) {
        self.answer(HoldAnswer::Choice(choice.into())).unwrap();
    }

    fn type_(&mut self, value: Json) {
        self.answer(HoldAnswer::Value(value.to_string())).unwrap();
    }

    /// The print lab answers its oldest request with `answer`.
    fn lab_answers(&mut self, answer: Json) {
        let ticket = self.lab.remove(0);
        let outcome = Outcome::Response(Response {
            status: 200,
            headers: Vec::new(),
            body: answer.to_string().into_bytes(),
        });
        self.r.fulfill(ticket, outcome).unwrap();
        self.pump();
    }

    fn calls(&self, source: &str) -> Vec<Json> {
        (self.tools.calls.lock().unwrap().iter())
            .filter(|(s, _)| s == source)
            .map(|(_, a)| a.clone())
            .collect()
    }

    fn ordered(&self) -> String {
        format!("{:?}", self.r.slot("ordered"))
    }

    fn logs(&self) -> String {
        exact_runner::agent::handle(&self.r, r#"{"op":"logs"}"#)
    }

    fn extensions(&self) -> String {
        self.r.extensions_json().unwrap_or_default()
    }
}

fn address() -> Json {
    json!({ "country": "US", "region": "CA", "city": "Palo Alto", "postalCode": "94301",
        "addressLine": ["1 Fixture Way"], "recipient": "Fixture Payer", "phone": "+15555550100" })
}

/// §9: the sheet is one ticket; an address asks `quoteShipping` with the
/// W3C's redaction and shows its update; `authorize` gives `placeOrder` a
/// synthetic payment in Apple's shape; the send answers its result with
/// the full address and no payment, which nothing records.
#[test]
fn a_sheet_asks_the_app_and_answers_its_result() {
    let mut host = Host::photo_editor("sheet", "ios");
    host.act("buy");
    let (_, args) = host.sheet().expect("a payment ticket");
    assert_eq!(args["method"], "https://apple.com/apple-pay", "{args}");
    assert_eq!(args["total"]["amount"]["value"], "8.00");
    assert_eq!(args["phase"], "editing");
    assert_eq!(args["requested"]["shipping"], true);
    host.type_(json!({ "shippingAddress": address() }));
    let quoted = host.calls("tool:quoteShipping");
    assert_eq!(quoted.len(), 1, "{}", host.logs());
    let shown = &quoted[0]["shippingAddress"];
    assert_eq!(shown["postalCode"], "94301");
    for field in ["addressLine", "recipient", "phone"] {
        assert!(shown[field].is_null(), "{field} is redacted: {shown}");
    }
    let (_, args) = host.sheet().unwrap();
    assert_eq!(args["total"]["amount"]["value"], "13.00", "{args}");
    assert_eq!(args["shippingOption"], "standard");
    host.type_(json!({ "shippingOption": "express" }));
    assert_eq!(
        host.calls("tool:quoteShipping")[1]["shippingOption"],
        "express"
    );
    host.tap("authorize");
    let placed = host.calls("tool:placeOrder");
    assert_eq!(placed.len(), 1, "{}", host.logs());
    let payment = &placed[0]["details"]["token"];
    assert_eq!(payment["paymentData"]["exactSubstitute"], true, "{payment}");
    assert_eq!(
        placed[0]["shippingAddress"]["addressLine"][0],
        "1 Fixture Way"
    );
    assert_eq!(placed[0]["payerEmail"], "payer@fixture.test");
    assert!(host.sheet().is_none(), "Apple's sheet closes on success");
    let ordered = host.ordered();
    assert!(
        ordered.contains("Success") || ordered.contains("success"),
        "{ordered}"
    );
    assert!(ordered.contains("PR-1"), "{ordered}");
    assert!(ordered.contains("Visa 0000"), "{ordered}");
    assert!(
        !ordered.contains("exactSubstitute"),
        "no payment in the result: {ordered}"
    );
    let (logs, extensions) = (host.logs(), host.extensions());
    for leak in [
        "exactSubstitute",
        "paymentData",
        "1 Fixture Way",
        "Fixture Payer",
    ] {
        assert!(!logs.contains(leak), "{leak} in the journal: {logs}");
        assert!(!extensions.contains(leak), "{leak} in state: {extensions}");
    }
    assert!(extensions.contains("\"handlers\""), "{extensions}");
    assert!(extensions.contains("placeOrder"), "{extensions}");
}

/// D6: an update's `error` is shown and the sheet stays; a retry shows its
/// field errors and the sheet waits for another authorization.
#[test]
fn an_error_update_and_a_retry_keep_the_sheet() {
    let mut host = Host::photo_editor("retry", "ios");
    host.act("buy");
    host.type_(json!({ "shippingAddress": { "country": "ZZ" } }));
    let (_, args) = host.sheet().unwrap();
    assert_eq!(args["error"], "We don't ship there.", "{args}");
    assert_eq!(args["shippingAddressErrors"]["country"], "We ship to US.");
    host.type_(json!({ "shippingAddress": address() }));
    *host.tools.mode.lock().unwrap() = "retry";
    host.tap("authorize");
    let (_, args) = host.sheet().expect("the sheet stays for the retry");
    assert_eq!(
        args["shippingAddressErrors"]["postalCode"], "Check the ZIP code.",
        "{args}"
    );
    assert_eq!(args["phase"], "editing");
    *host.tools.mode.lock().unwrap() = "success";
    host.tap("authorize");
    assert!(host.sheet().is_none(), "{}", host.logs());
    assert_eq!(host.calls("tool:placeOrder").len(), 2);
    let first = &host.calls("tool:placeOrder")[0]["details"]["token"]["transactionIdentifier"];
    let second = &host.calls("tool:placeOrder")[1]["details"]["token"]["transactionIdentifier"];
    assert_ne!(first, second, "each authorization is a new payment");
}

/// D6's table: Google's error result keeps its sheet up, so the send waits
/// for the next authorization; Apple's closes with the failure.
#[test]
fn a_failure_closes_apples_sheet_and_keeps_googles() {
    let mut apple = Host::photo_editor("fail-apple", "ios");
    *apple.tools.mode.lock().unwrap() = "fail";
    apple.act("buy");
    apple.tap("authorize");
    assert!(apple.sheet().is_none());
    assert!(apple.ordered().contains("ail"), "{}", apple.ordered());

    let mut google = Host::photo_editor("fail-google", "android");
    *google.tools.mode.lock().unwrap() = "fail";
    google.act("buy");
    let (_, args) = google.sheet().unwrap();
    assert_eq!(args["method"], "https://google.com/pay");
    google.tap("authorize");
    let token =
        &google.calls("tool:placeOrder")[0]["details"]["paymentMethodData"]["tokenizationData"];
    assert_eq!(token["type"], "PAYMENT_GATEWAY", "{token}");
    assert!(token["token"].as_str().unwrap().contains("exactSubstitute"));
    let (_, args) = google.sheet().expect("Google's sheet stays up");
    assert_eq!(args["phase"], "editing");
    assert_eq!(args["result"], "fail");
    assert!(
        !google.ordered().contains("ail"),
        "unanswered: {}",
        google.ordered()
    );
    *google.tools.mode.lock().unwrap() = "success";
    google.tap("authorize");
    assert!(google.sheet().is_none());
    assert!(google.ordered().contains("PR-1"), "{}", google.ordered());
    assert!(google.ordered().contains("VISA"), "{}", google.ordered());
}

/// D5, D16 item 2: a send let go while its final handler runs is not
/// cancelled; the handler's answer then closes the sheet and answers nothing.
#[test]
fn a_running_final_handler_outlives_its_send() {
    let mut host = Host::photo_editor("final", "ios");
    *host.tools.mode.lock().unwrap() = "hold";
    host.act("buy");
    host.tap("authorize");
    assert_eq!(
        host.lab.len(),
        1,
        "placeOrder is at the print lab: {}",
        host.logs()
    );
    host.act("leave");
    assert!(
        host.logs().contains("held until the handler answers"),
        "{}",
        host.logs()
    );
    assert!(!host.logs().contains("its call was let go\n") || host.sheet().is_some());
    host.lab_answers(json!({ "result": "success", "data": { "orderId": "PR-9" } }));
    assert!(host.sheet().is_none(), "{}", host.logs());
    assert!(
        host.logs().contains("closed after its call ended, success"),
        "{}",
        host.logs()
    );
    assert!(!host.ordered().contains("PR-9"), "{}", host.ordered());
}

/// D6: let go while editing, Apple's sheet closes; Google's cannot, so it
/// is abandoned, and its authorization fails without calling the app.
#[test]
fn a_send_let_go_closes_or_abandons_by_target() {
    let mut apple = Host::photo_editor("leave-apple", "ios");
    apple.act("buy");
    apple.act("leave");
    assert!(apple.sheet().is_none(), "{}", apple.logs());
    assert!(
        apple.logs().contains("closed, its call was let go"),
        "{}",
        apple.logs()
    );

    let mut google = Host::photo_editor("leave-google", "android");
    google.act("buy");
    google.act("leave");
    let (_, args) = google.sheet().expect("abandoned, still up");
    assert_eq!(args["phase"], "abandoned", "{args}");
    google.tap("authorize");
    assert!(
        google.calls("tool:placeOrder").is_empty(),
        "{}",
        google.logs()
    );
    assert!(
        google.logs().contains("no handler run"),
        "{}",
        google.logs()
    );
    google.tap("cancel");
    assert!(google.sheet().is_none());
}

/// D5: a handler past its budget on the session's clock: the authorization
/// fails `authorize-timeout`, and its late answer is dropped.
#[test]
fn a_handler_past_its_budget_fails_the_authorization() {
    let mut host = Host::photo_editor("budget", "ios");
    *host.tools.mode.lock().unwrap() = "hold";
    host.act("buy");
    host.tap("authorize");
    let now = host.r.now_ms();
    host.r.advance(now + 25_001.0).unwrap();
    host.pump();
    assert!(
        host.ordered().contains("authorize-timeout"),
        "{}\n{}",
        host.ordered(),
        host.logs()
    );
    host.lab_answers(json!({ "result": "success" }));
    assert!(host.logs().contains("dropped"), "{}", host.logs());
}

/// D14: a tool's call cannot spend; no sheet is held.
#[test]
fn a_tool_cannot_pay() {
    let mut host = Host::photo_editor("tool", "ios");
    host.r.act_from(CallOrigin::Tool, "buy", vec![]).unwrap();
    host.pump();
    assert!(host.sheet().is_none(), "{}", host.logs());
    assert!(host.logs().contains("person-required"), "{}", host.logs());
}

/// D8: on the web, Apple Pay validates the merchant before the sheet takes
/// an answer, through the app's `onmerchantvalidation`.
#[test]
fn the_web_validates_apple_pays_merchant_first() {
    let mut host = Host::photo_editor("web", "web");
    host.act("buy");
    let validated = host.calls("tool:validateMerchant");
    assert_eq!(validated.len(), 1, "{}", host.logs());
    assert!(validated[0]["validationURL"]
        .as_str()
        .unwrap()
        .starts_with("https://apple-pay-gateway-cert.apple.com/"));
    let (_, args) = host.sheet().unwrap();
    assert_eq!(args["phase"], "editing", "{args}");
    let bare = app(
        MERCHANT,
        GOOGLE,
        "USD",
        r#", onpaymentauthorized="placeOrder""#,
    );
    let mut host = Host::boot("web-bare", "web", &bare, GRANTS);
    host.act("buy");
    assert!(host.sheet().is_none());
    assert!(
        host.ordered().is_empty() || !host.ordered().contains("PR-"),
        "{}",
        host.ordered()
    );
    assert!(
        host.logs().contains("payment-merchant-validation"),
        "{}",
        host.logs()
    );
}

/// D10, D13: what the call is refused, and the code it is refused with,
/// before any sheet: an ungranted merchant ID, a currency that is no ISO
/// 4217 code, `environment` (the bake's), and a target with no wallet.
#[test]
fn a_request_is_checked_before_its_sheet() {
    let cases = [
        (
            "merchant",
            app("merchant.com.other", GOOGLE, "USD", HANDLERS),
            "ios",
            "SecurityError",
        ),
        (
            "currency",
            app(MERCHANT, GOOGLE, "US", HANDLERS),
            "ios",
            "RangeError",
        ),
        (
            "environment",
            app(
                MERCHANT,
                &GOOGLE.replace(
                    r#"{\"apiVersion\""#,
                    r#"{\"environment\":\"TEST\",\"apiVersion\""#,
                ),
                "USD",
                HANDLERS,
            ),
            "android",
            "payment-environment",
        ),
        (
            "linux",
            app(MERCHANT, GOOGLE, "USD", HANDLERS),
            "linux",
            "NotSupportedError",
        ),
    ];
    for (name, contract, target, code) in cases {
        let mut host = Host::boot(name, target, &contract, GRANTS);
        host.act("buy");
        assert!(host.sheet().is_none(), "{name}: {}", host.logs());
        assert!(
            host.logs().contains(code),
            "{name}: {code} in {}",
            host.logs()
        );
    }
    let mut host = Host::boot(
        "ungranted",
        "android",
        &app(MERCHANT, GOOGLE, "USD", HANDLERS),
        "payment.apple-pay merchant.com.exact.photoeditor\n",
    );
    host.act("buy");
    assert!(host.logs().contains("grant-missing"), "{}", host.logs());
}
