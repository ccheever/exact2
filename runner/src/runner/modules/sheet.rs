//! A sheet that asks the app while it is up (LLP 1133.034 §9, D16 item 3):
//! a ceremony whose substitute declares `asks` holds one ticket for the
//! wallet's sheet, which the agent answers as the person would, and the app's
//! own handlers answer the sheet as they would the platform's.
//!
//! - **Asks.** `type @t {"shippingAddress": {…}}` (redacted by the W3C's
//!   rule before the handler sees it), `{"shippingOption": "<id>"}` and
//!   `{"paymentMethod": {…}}` each call the handler its `asks` entry names
//!   through `invoke` (`run_handler`), and lay its `PaymentDetailsUpdate`
//!   over the ticket's `args`. `tap @t authorize` calls the final handler
//!   with a synthetic payment in the method's own shape (D7), and its answer
//!   is shown as D6's table shows it on this target. On the web with Apple
//!   Pay, merchant validation is asked first.
//! - **One at a time.** While a handler runs, the sheet takes no other
//!   answer, as Apple's delegate waits for each before the next.
//! - **The budget** (`budget`, ms) runs on the session's clock: past it an
//!   update leaves the sheet as it was with the error `unavailable`, and an
//!   authorization fails with the message `authorize-timeout`. A late
//!   answer is journaled and dropped.
//! - **Let go.** A Contract send let go while authorizing is held until the
//!   handler answers, and then answers nothing. One let go while editing
//!   closes where the platform can (Apple; the web outside a retry), else is
//!   abandoned: its ticket stays until `cancel`, and an authorization then
//!   fails without calling the app.
//! - **Never a token** in the ticket, the journal or `state.extensions`: a
//!   handler's argument is held only while it runs (`invoke.rs`).

use super::super::{DataSource, Runner};
use super::ceremony::within;
use super::invoke::To;
use super::{CallOrigin, Json, ModulesRow};
use crate::{Hold, HoldAnswer};
use serde_json::json;
use std::collections::BTreeMap;

const APPLE: &str = "https://apple.com/apple-pay";
const GOOGLE: &str = "https://google.com/pay";
/// Apple's sandbox gateway, the host of the validation URL Safari would give.
const VALIDATION_URL: &str =
    "https://apple-pay-gateway-cert.apple.com/paymentservices/startSession";
/// The payer the substitute's sheet fills in for requested contact fields.
const PAYER: (&str, &str, &str) = ("Fixture Payer", "payer@fixture.test", "+15555550100");

#[derive(Default)]
pub(super) struct Sheets {
    /// By the held call's token.
    open: BTreeMap<u64, Sheet>,
    /// Synthetic payments made, for each one's transaction identifier.
    payments: u64,
    /// Handler calls made, each one's number.
    calls: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Merchant validation, before the person can act.
    Validating,
    Editing,
    Authorizing,
    /// Let go where the platform cannot close the sheet (D6 phase 3).
    Abandoned,
}

impl Phase {
    fn word(self) -> &'static str {
        match self {
            Phase::Validating => "validating",
            Phase::Editing => "editing",
            Phase::Authorizing => "authorizing",
            Phase::Abandoned => "abandoned",
        }
    }
}

/// A handler the sheet waits for.
struct Asking {
    ask: String,
    handler: String,
    call: u64,
    started: f64,
    ends: f64,
}

struct Sheet {
    hold: u64,
    name: String,
    request_id: String,
    method: String,
    /// The handler each keyword names: `onpaymentauthorized` → `placeOrder`.
    handlers: BTreeMap<String, String>,
    origin: CallOrigin,
    /// `total`, `displayItems`, `shippingOptions` and `modifiers`, as shown.
    details: Json,
    options: Json,
    /// `error` and the field errors the last handler gave.
    errors: Json,
    /// The last address typed, in full; the shipping option chosen.
    address: Json,
    option: Json,
    phase: Phase,
    retrying: bool,
    /// The last authorization's result, message and data.
    last: Option<Json>,
    let_go: bool,
    /// The send was answered; the ticket is the sheet's alone (an Android
    /// sheet whose authorization timed out).
    answered: bool,
    asking: Option<Asking>,
    /// Each handler's name, duration and answer kind (`state.extensions`).
    handled: Vec<Json>,
    budget: f64,
    /// The words the result names the card by (`apple`, `android`, D10).
    card: Json,
}

/// `address` as `shippingaddresschange` gives it (W3C §`redact list`).
fn redacted(address: &Json) -> Json {
    let mut out = address.clone();
    if let Some(object) = out.as_object_mut() {
        for field in ["organization", "phone", "recipient", "addressLine"] {
            object.remove(field);
        }
    }
    out
}

/// A `PaymentCurrencyAmount`'s refusal: a currency of three letters, a
/// value a decimal string (the W3C's "valid decimal monetary value").
fn amount_refused(amount: &Json, what: &str) -> Option<(&'static str, String)> {
    let currency = amount["currency"].as_str().unwrap_or_default();
    if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Some((
            "RangeError",
            format!("{what}'s currency `{currency}` is no ISO 4217 code"),
        ));
    }
    let value = amount["value"].as_str().unwrap_or_default();
    let digits = value.strip_prefix('-').unwrap_or(value);
    let (whole, part) = digits.split_once('.').unwrap_or((digits, "0"));
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !numeric(whole) || !numeric(part) {
        return Some((
            "TypeError",
            format!("{what}'s value `{value}` is no decimal string"),
        ));
    }
    None
}

impl<D: DataSource> Runner<D> {
    /// The method this target pays with: the first of `args`'s method data
    /// whose declared `targets` name it (D4).
    fn method_here(&self, row: &ModulesRow, args: &Json) -> Option<String> {
        let methods = self.substitute(row)["methods"].clone();
        let target = self.host_target();
        (args["methodData"].as_array().into_iter().flatten())
            .filter_map(|m| m["supportedMethods"].as_str())
            .find(|m| {
                (methods[*m]["targets"].as_array()).is_some_and(|t| t.iter().any(|x| x == target))
            })
            .map(str::to_owned)
    }

    /// A payment call's method data, details and amounts, as the W3C's
    /// constructor and each method's dictionaries check them (D10, D16 item
    /// 4): the code and why it is refused, if it is.
    pub(super) fn methods_admitted(
        &self,
        row: &ModulesRow,
        args: &Json,
    ) -> Result<(), (Json, String)> {
        let sub = self.substitute(row);
        let Some(methods) = sub["methods"].as_object() else {
            return Ok(());
        };
        let refuse = |code: &str, why: String| Err((json!(code), why));
        let data = args["methodData"].as_array().cloned().unwrap_or_default();
        if data.is_empty() {
            return refuse("TypeError", "methodData is empty".into());
        }
        let keys_of = |value: Option<Json>, allowed: &Json, method: &str, place: &str| {
            let Some(value) = value else {
                return Ok(());
            };
            let Some(object) = value.as_object() else {
                return Err((
                    json!("TypeError"),
                    format!("{method}'s {place} is no object"),
                ));
            };
            for key in object.keys() {
                if let Some(code) = methods[method]["refuse"][key].as_str() {
                    let why = format!("{code}: `{key}` is the bake's, never the app's");
                    return Err((json!("TypeError"), why));
                }
                if !(allowed.as_array().into_iter().flatten()).any(|k| k == key) {
                    let why = format!("`{key}` is no key of {method}'s {place}");
                    return Err((json!("TypeError"), why));
                }
            }
            Ok(())
        };
        let grants = self.data.grants();
        for m in &data {
            let method = m["supportedMethods"].as_str().unwrap_or_default();
            let Some(declared) = methods.get(method) else {
                continue;
            };
            keys_of(within(m, "data"), &declared["data"], method, "data")?;
            let line = declared["grant"].as_str().unwrap_or_default();
            if !grants
                .lines()
                .any(|l| l.split_whitespace().next() == Some(line))
            {
                return refuse(
                    "NotSupportedError",
                    format!("grant-missing ({method} needs `{line}`)"),
                );
            }
        }
        let details = &args["details"];
        let mut amounts = vec![("total", details["total"]["amount"].clone())];
        for (list, what) in [
            ("displayItems", "a display item"),
            ("shippingOptions", "a shipping option"),
        ] {
            for item in details[list].as_array().into_iter().flatten() {
                amounts.push((what, item["amount"].clone()));
            }
        }
        let mut exclusive = Vec::new();
        for modifier in details["modifiers"].as_array().into_iter().flatten() {
            let method = modifier["supportedMethods"].as_str().unwrap_or_default();
            if !modifier["total"].is_null() {
                amounts.push(("a modifier's total", modifier["total"]["amount"].clone()));
            }
            let Some(declared) = methods.get(method) else {
                continue;
            };
            let data = within(modifier, "data");
            keys_of(data.clone(), &declared["modifier"], method, "modifier")?;
            for key in declared["exclusive"].as_array().into_iter().flatten() {
                let key = key.as_str().unwrap_or_default();
                if data.as_ref().is_some_and(|d| !d[key].is_null()) && !exclusive.contains(&key) {
                    exclusive.push(key);
                }
            }
        }
        if exclusive.len() > 1 {
            let why = format!("{} exclude one another (PassKit)", exclusive.join(" and "));
            return refuse("TypeError", why);
        }
        for (what, amount) in amounts {
            if let Some((code, why)) = amount_refused(&amount, what) {
                return refuse(code, why);
            }
        }
        if details["total"]["amount"]["value"]
            .as_str()
            .is_some_and(|v| v.starts_with('-'))
        {
            return refuse("TypeError", "the total is negative".into());
        }
        if self.method_here(row, args).is_none() {
            let why = format!("no method in methodData that {} has", self.host_target());
            return refuse("NotSupportedError", why);
        }
        Ok(())
    }

    /// A sheet for a call of `row` with `args`, before it is opened.
    fn sheet_of(&self, row: &ModulesRow, args: &Json, origin: CallOrigin, id: String) -> Sheet {
        let sub = self.substitute(row);
        let handlers = (sub["callbacks"].as_object().into_iter().flatten())
            .filter_map(|(k, _)| Some((k.clone(), args[k].as_str()?.to_owned())))
            .collect();
        let details = &args["details"];
        let option = (details["shippingOptions"].as_array().into_iter().flatten())
            .rfind(|o| o["selected"] == true)
            .map_or(Json::Null, |o| o["id"].clone());
        Sheet {
            hold: 0,
            name: String::new(),
            request_id: details["id"].as_str().map_or(id, str::to_owned),
            method: self.method_here(row, args).unwrap_or_default(),
            handlers,
            origin,
            details: json!({
                "total": details["total"],
                "displayItems": details["displayItems"],
                "shippingOptions": details["shippingOptions"],
                "modifiers": details["modifiers"],
            }),
            options: args["options"].clone(),
            errors: json!({}),
            address: Json::Null,
            option,
            phase: Phase::Editing,
            retrying: false,
            last: None,
            let_go: false,
            answered: false,
            asking: None,
            handled: Vec::new(),
            budget: sub["ceremony"]["budget"].as_f64().unwrap_or(25_000.0),
            card: Json::Null,
        }
    }

    /// The ticket's `args` (§9): never an address, a payer or a token.
    fn args_shown(&self, sheet: &Sheet) -> Json {
        let o = &sheet.options;
        json!({
            "request": "showPaymentRequest",
            "ceremony": "showPaymentRequest",
            "requestId": sheet.request_id,
            "method": sheet.method,
            "total": sheet.details["total"],
            "displayItems": sheet.details["displayItems"],
            "shippingOptions": sheet.details["shippingOptions"],
            "shippingOption": sheet.option,
            "requested": {
                "payerName": o["requestPayerName"] == true,
                "payerEmail": o["requestPayerEmail"] == true,
                "payerPhone": o["requestPayerPhone"] == true,
                "shipping": o["requestShipping"] == true,
                "billingAddress": o["requestBillingAddress"] == true,
            },
            "error": sheet.errors["error"],
            "shippingAddressErrors": sheet.errors["shippingAddressErrors"],
            "payerErrors": sheet.errors["payerErrors"],
            "paymentMethodErrors": sheet.errors["paymentMethodErrors"],
            "phase": sheet.phase.word(),
            "waiting": sheet.asking.as_ref().map(|a| a.ask.clone()),
            "result": sheet.last.as_ref().map(|l| l["result"].clone()),
            "origin": sheet.origin.word(),
            "target": self.host_target(),
        })
    }

    /// A payment ticket's `args` as it is first held.
    pub(super) fn sheet_args_of(&self, row: &ModulesRow, args: &Json, origin: CallOrigin) -> Json {
        self.args_shown(&self.sheet_of(row, args, origin, String::new()))
    }

    /// The call held under `token` has its ticket: its sheet opens, and on
    /// the web with Apple Pay first, merchant validation is asked.
    pub(super) fn sheet_opened(&mut self, token: u64) {
        let Some(h) = self.extensions.held.iter().find(|h| h.token == token) else {
            return;
        };
        let row = self.module_row(&h.source);
        if !self.substitute(&row)["ceremony"]["asks"].is_object() {
            return;
        }
        let (hold, args, origin) = (h.hold, h.args.clone(), h.origin);
        self.extensions.sheets.calls += 1;
        let id = format!("payment-{}", self.extensions.sheets.calls);
        let mut sheet = self.sheet_of(&row, &args, origin, id);
        sheet.hold = hold;
        sheet.name = (self.device_holds.iter())
            .find(|d| d.ticket == hold)
            .map_or_else(|| "payment".to_owned(), |d| d.name.clone());
        let validates = self.host_target() == "web" && sheet.method == APPLE;
        let validator = sheet.handlers.get("onmerchantvalidation").cloned();
        self.log(format!(
            "payment {}: held, {} {}",
            sheet.request_id,
            sheet.method,
            amount_text(&sheet.details["total"]["amount"])
        ));
        let request_id = sheet.request_id.clone();
        self.extensions.sheets.open.insert(token, sheet);
        if !validates {
            return self.reopen(token);
        }
        if validator.is_none() {
            let why =
                "payment-merchant-validation: Apple Pay on the web needs onmerchantvalidation";
            return self.sheet_ends(token, json!({ "error": "OperationError", "message": why }));
        }
        self.sheet_mut(token).phase = Phase::Validating;
        let arg = json!({ "requestId": request_id, "methodName": APPLE, "validationURL": VALIDATION_URL });
        self.sheet_call(token, "merchantValidation", "onmerchantvalidation", arg);
    }

    fn sheet_mut(&mut self, token: u64) -> &mut Sheet {
        self.extensions
            .sheets
            .open
            .get_mut(&token)
            .expect("an open sheet")
    }

    /// The ticket shows the sheet as it is now, held again if an answer spent it.
    fn reopen(&mut self, token: u64) {
        let Some(sheet) = self.extensions.sheets.open.get(&token) else {
            return;
        };
        let args = self.args_shown(sheet).to_string();
        let (ticket, name) = (sheet.hold, sheet.name.clone());
        match self.device_holds.iter_mut().find(|h| h.ticket == ticket) {
            Some(h) => h.args = args,
            None => self.device_holds.push(Hold {
                ticket,
                capability: "payment".into(),
                name,
                node: None,
                args,
                choices: vec!["authorize".into()],
                takes_value: true,
                request: false,
            }),
        }
    }

    /// Call the handler `keyword` names for `ask` with `arg`; `false` when
    /// the call names none.
    fn sheet_call(&mut self, token: u64, ask: &str, keyword: &str, arg: Json) -> bool {
        let sheet = self.sheet_mut(token);
        let Some(handler) = sheet.handlers.get(keyword).cloned() else {
            return false;
        };
        let (origin, budget, id) = (sheet.origin, sheet.budget, sheet.request_id.clone());
        self.extensions.sheets.calls += 1;
        let call = self.extensions.sheets.calls;
        let now = self.now_ms;
        self.sheet_mut(token).asking = Some(Asking {
            ask: ask.to_owned(),
            handler: handler.clone(),
            call,
            started: now,
            ends: now + budget,
        });
        self.reopen(token);
        let origin = origin.min(CallOrigin::Tool);
        let at = format!("payments: {handler} for {id}");
        self.log(format!("{at}: run, {}", origin.word()));
        let to = To::Sheet {
            token,
            attempt: call,
        };
        self.run_handler(at, &handler, &arg, origin, to, None);
        true
    }

    /// A handler of the sheet held under `token` answered `answer`, `{"text"}`
    /// or `{"error"}`, for its call `call`.
    pub(super) fn sheet_handled(&mut self, token: u64, call: u64, answer: Json) {
        let asking = self.extensions.sheets.open.get_mut(&token).and_then(|s| {
            match s.asking.as_ref().is_some_and(|a| a.call == call) {
                true => s.asking.take(),
                false => None,
            }
        });
        let Some(asking) = asking else {
            self.log(format!(
                "payments: handler call {call} answered after its sheet moved on; dropped"
            ));
            return;
        };
        let value = (answer["text"].as_str()).and_then(|t| serde_json::from_str::<Json>(t).ok());
        let kind = match (&value, asking.ask.as_str()) {
            (None, _) => "error".to_owned(),
            (Some(v), "authorize") if !v["retry"].is_null() => "retry".to_owned(),
            (Some(v), "authorize") => v["result"].as_str().unwrap_or("malformed").to_owned(),
            (Some(_), _) => "update".to_owned(),
        };
        let ms = self.now_ms - asking.started;
        let record = json!({ "name": asking.handler, "ms": ms, "answer": kind });
        self.sheet_mut(token).handled.push(record);
        match asking.ask.as_str() {
            "merchantValidation" => {
                let session = value.as_ref().map(|v| &v["merchantSession"]);
                match session.is_some_and(|s| s.as_object().is_some_and(|o| !o.is_empty())) {
                    true => {
                        self.sheet_mut(token).phase = Phase::Editing;
                        self.reopen(token);
                    }
                    false => {
                        let why = "merchant validation answered no merchant session";
                        self.sheet_ends(
                            token,
                            json!({ "error": "OperationError", "message": why }),
                        );
                    }
                }
            }
            "authorize" => match value {
                Some(v) if v["retry"].is_object() => {
                    let s = self.sheet_mut(token);
                    let r = &v["retry"];
                    s.errors = json!({
                        "error": r["error"],
                        "shippingAddressErrors": r["shippingAddress"],
                        "payerErrors": r["payer"],
                        "paymentMethodErrors": r["paymentMethod"],
                    });
                    s.retrying = true;
                    s.phase = if s.let_go {
                        Phase::Abandoned
                    } else {
                        Phase::Editing
                    };
                    self.reopen(token);
                }
                Some(v) if matches!(v["result"].as_str(), Some("success" | "fail" | "unknown")) => {
                    let result = v["result"].as_str().unwrap_or_default().to_owned();
                    self.sheet_result(token, &result, None, v["data"].clone());
                }
                _ => self.sheet_result(token, "fail", Some("authorize-timeout"), Json::Null),
            },
            _ => {
                match value {
                    Some(update) => self.sheet_updated(token, &update),
                    None => self.sheet_mut(token).errors = json!({ "error": "unavailable" }),
                }
                self.reopen(token);
            }
        }
    }

    /// A `PaymentDetailsUpdate` laid over the sheet.
    fn sheet_updated(&mut self, token: u64, update: &Json) {
        let s = self.sheet_mut(token);
        for field in ["total", "displayItems", "shippingOptions", "modifiers"] {
            if !update[field].is_null() {
                s.details[field] = update[field].clone();
            }
        }
        if let Some(options) = update["shippingOptions"].as_array() {
            s.option = (options.iter().rfind(|o| o["selected"] == true))
                .map_or(Json::Null, |o| o["id"].clone());
        }
        s.errors = json!({
            "error": update["error"],
            "shippingAddressErrors": update["shippingAddressErrors"],
            "payerErrors": update["payerErrors"],
            "paymentMethodErrors": update["paymentMethodErrors"],
        });
    }

    /// The authorization's result, as D6's table shows it on this target.
    fn sheet_result(&mut self, token: u64, result: &str, message: Option<&str>, data: Json) {
        let android = self.host_target() == "android";
        let closes = result == "success" || !android;
        let s = self.sheet_mut(token);
        s.last = Some(json!({ "result": result, "message": message, "data": data }));
        let id = s.request_id.clone();
        self.log(format!(
            "payment {id}: {result}{}",
            message.map(|m| format!(", {m}")).unwrap_or_default()
        ));
        let s = self.sheet_mut(token);
        if closes {
            return match s.let_go {
                true => {
                    self.sheet_dropped(token, &format!("closed after its call ended, {result}"))
                }
                false => {
                    let answer = self.payment_result(token);
                    self.sheet_ends(token, answer);
                }
            };
        }
        // Google's error result keeps the sheet up (back to editing), unless
        // its call ended or its authorization timed out (abandoned).
        if s.let_go {
            s.phase = Phase::Abandoned;
        } else if message == Some("authorize-timeout") {
            s.phase = Phase::Abandoned;
            s.answered = true;
            let answer = self.payment_result(token);
            self.answer_send(token, answer);
        } else {
            s.phase = Phase::Editing;
        }
        self.reopen(token);
    }

    /// The send's `PaymentResult`: the response without its payment (D7).
    fn payment_result(&self, token: u64) -> Json {
        let s = &self.extensions.sheets.open[&token];
        let o = &s.options;
        let last = s.last.clone().unwrap_or_default();
        let data = match &last["data"] {
            Json::Null => Json::Null,
            d => json!(d.to_string()),
        };
        let mut out = json!({
            "requestId": s.request_id,
            "methodName": s.method,
            "shippingAddress": if o["requestShipping"] == true { s.address.clone() } else { Json::Null },
            "shippingOption": if o["requestShipping"] == true { s.option.clone() } else { Json::Null },
            "payerName": (o["requestPayerName"] == true).then_some(PAYER.0),
            "payerEmail": (o["requestPayerEmail"] == true).then_some(PAYER.1),
            "payerPhone": (o["requestPayerPhone"] == true).then_some(PAYER.2),
            "result": last["result"],
            "message": last["message"],
            "data": data,
        });
        let words = if s.method == APPLE {
            "apple"
        } else {
            "android"
        };
        if !s.card.is_null() {
            out[words] = s.card.clone();
        }
        out
    }

    /// The sheet closes and the send answers `answer`.
    fn sheet_ends(&mut self, token: u64, answer: Json) {
        let Some(s) = self.extensions.sheets.open.get(&token) else {
            return;
        };
        let (hold, answered) = (s.hold, s.answered);
        self.retire_hold(hold);
        if answered {
            self.extensions.sheets.open.remove(&token);
            return;
        }
        self.release(token, answer);
        self.extensions.sheets.open.remove(&token);
    }

    /// The send answers `answer` while its sheet stays up (D6: an Android
    /// sheet whose authorization timed out).
    fn answer_send(&mut self, token: u64, answer: Json) {
        let hold = self.extensions.sheets.open[&token].hold;
        // The ticket is the sheet's now; the call's own retires with its answer.
        let ticket = self.device_holds.iter().position(|h| h.ticket == hold);
        let kept = ticket.map(|at| self.device_holds.remove(at));
        self.release(token, answer);
        if let Some(kept) = kept {
            self.device_holds.push(kept);
        }
    }

    /// A sheet whose call was let go closes: its ticket retires, and the send
    /// answers nothing.
    fn sheet_dropped(&mut self, token: u64, why: &str) {
        let Some(s) = self.extensions.sheets.open.get(&token) else {
            return;
        };
        let (hold, id) = (s.hold, s.request_id.clone());
        let last = s
            .last
            .as_ref()
            .map(|l| l["result"].clone())
            .unwrap_or(Json::Null);
        self.log(format!("payment: {why} ({id}, last result {last})"));
        self.retire_hold(hold);
        if let Some(at) = self.extensions.held.iter().position(|h| h.token == token) {
            let held = self.extensions.held.remove(at);
            let row = self.module_row(&held.source);
            self.record_ceremony(token, &row, &held.args, "closed after its call ended");
        }
        self.extensions.sheets.open.remove(&token);
        self.promote_ceremony();
    }

    /// A typed answer to a sheet, checked before its hold is spent.
    pub(super) fn sheet_typed(&self, token: u64, text: &str) -> Result<Json, String> {
        let form = "type one of {\"shippingAddress\": {…}}, {\"shippingOption\": \"<id>\"} or {\"paymentMethod\": {…}}";
        let value: Json = serde_json::from_str(text.trim()).map_err(|_| form.to_owned())?;
        let object = value.as_object().filter(|o| o.len() == 1).ok_or(form)?;
        let s = self
            .extensions
            .sheets
            .open
            .get(&token)
            .ok_or("payment: no sheet is up")?;
        if let Some(asking) = &s.asking {
            return Err(format!("payment: the sheet waits for {}", asking.handler));
        }
        if matches!(s.phase, Phase::Authorizing | Phase::Validating) {
            return Err(format!("payment: the sheet is {}", s.phase.word()));
        }
        match object.iter().next() {
            Some((k, v)) if k == "shippingAddress" || k == "paymentMethod" => match v.is_object() {
                true => Ok(value),
                false => Err(format!("payment: `{k}` is an object")),
            },
            Some((k, v)) if k == "shippingOption" => {
                let listed = (s.details["shippingOptions"]
                    .as_array()
                    .into_iter()
                    .flatten())
                .any(|o| o["id"] == *v);
                match listed {
                    true => Ok(value),
                    false => Err(format!("payment: no shipping option {v} is shown")),
                }
            }
            _ => Err(form.to_owned()),
        }
    }

    /// The agent answered hold `hold`, already spent; `true` when it is a sheet's.
    pub(super) fn sheet_answered_hold(&mut self, hold: u64, answer: &HoldAnswer) -> bool {
        let Some(token) = (self.extensions.sheets.open.iter())
            .find(|(_, s)| s.hold == hold)
            .map(|(t, _)| *t)
        else {
            return false;
        };
        match answer {
            HoldAnswer::Choice(c) if c == "cancel" => self.sheet_cancelled(token),
            HoldAnswer::Choice(c) if c == "authorize" => self.sheet_authorize(token),
            HoldAnswer::Choice(_) => self.reopen(token),
            HoldAnswer::Value(text) => self.sheet_asked(token, text),
        }
        true
    }

    /// `type @t {…}`: the handler for what the person changed.
    fn sheet_asked(&mut self, token: u64, text: &str) {
        let Ok(value) = self.sheet_typed(token, text) else {
            return self.reopen(token);
        };
        let s = self.sheet_mut(token);
        // A request whose call ended calls the app no more (D6).
        let abandoned = s.phase == Phase::Abandoned;
        let (id, method) = (s.request_id.clone(), s.method.clone());
        let (ask, keyword, arg) = if let Some(address) = value.get("shippingAddress") {
            s.address = address.clone();
            let arg = json!({ "requestId": id, "shippingAddress": redacted(address), "shippingOption": s.option, "methodName": method });
            ("shippingAddress", "onshippingaddresschange", arg)
        } else if let Some(option) = value.get("shippingOption") {
            for o in (s.details["shippingOptions"].as_array_mut().into_iter()).flatten() {
                o["selected"] = json!(o["id"] == *option);
            }
            s.option = option.clone();
            let arg = json!({ "requestId": id, "shippingAddress": redacted(&s.address), "shippingOption": option, "methodName": method });
            ("shippingOption", "onshippingoptionchange", arg)
        } else {
            let arg = json!({ "requestId": id, "methodName": method, "methodDetails": value["paymentMethod"] });
            ("paymentMethod", "onpaymentmethodchange", arg)
        };
        if abandoned || !self.sheet_call(token, ask, keyword, arg) {
            self.reopen(token);
        }
    }

    /// `tap @t authorize`: the final handler, with a synthetic payment.
    fn sheet_authorize(&mut self, token: u64) {
        let s = self.sheet_mut(token);
        if s.asking.is_some() || matches!(s.phase, Phase::Authorizing | Phase::Validating) {
            return self.reopen(token);
        }
        if s.phase == Phase::Abandoned {
            let (id, web) = (s.request_id.clone(), self.host_target() == "web");
            self.log(format!(
                "payment {id}: authorized after its call ended; answered as failed, no handler run"
            ));
            // The web's `complete("fail")` after a retry closes it; Google's error keeps it up.
            return match web {
                true => self.sheet_dropped(token, "closed after its call ended, fail"),
                false => self.reopen(token),
            };
        }
        let response = self.payment_response(token);
        let s = self.sheet_mut(token);
        s.phase = Phase::Authorizing;
        s.errors = json!({});
        if !self.sheet_call(token, "authorize", "onpaymentauthorized", response) {
            self.sheet_result(token, "fail", Some("authorize-timeout"), Json::Null);
        }
    }

    /// The `PaymentResponse` the final handler is given: a synthetic payment
    /// in the method's own shape, never a card's (§9).
    fn payment_response(&mut self, token: u64) -> Json {
        self.extensions.sheets.payments += 1;
        let n = self.extensions.sheets.payments;
        let s = &self.extensions.sheets.open[&token];
        let marker = json!({ "exactSubstitute": true, "requestId": s.request_id, "payment": n });
        let (details, card) = if s.method == APPLE {
            let method = json!({ "displayName": "Visa 0000", "network": "Visa", "type": "credit" });
            let details = json!({
                "token": {
                    "paymentData": { "exactSubstitute": true, "version": "EC_v1", "data": marker.to_string() },
                    "paymentMethod": method,
                    "transactionIdentifier": format!("{n:064X}"),
                },
            });
            (details, json!({ "paymentMethod": method }))
        } else {
            let held = self.extensions.held.iter().find(|h| h.token == token);
            let google = (held
                .map(|h| h.args["methodData"].clone())
                .unwrap_or_default())
            .as_array()
            .into_iter()
            .flatten()
            .find(|m| m["supportedMethods"] == GOOGLE)
            .and_then(|m| within(m, "data"))
            .unwrap_or_default();
            let tokenizing =
                &google["allowedPaymentMethods"][0]["tokenizationSpecification"]["type"];
            let token_text = match tokenizing.as_str() {
                Some("DIRECT") => json!({
                    "protocolVersion": "ECv2",
                    "signature": "exact-substitute",
                    "intermediateSigningKey": { "signedKey": "{}", "signatures": ["exact-substitute"] },
                    "signedMessage": marker.to_string(),
                })
                .to_string(),
                _ => marker.to_string(),
            };
            let details = json!({
                "apiVersion": 2,
                "apiVersionMinor": 0,
                "paymentMethodData": {
                    "type": "CARD",
                    "description": "Visa •••• 1111",
                    "info": { "cardNetwork": "VISA", "cardDetails": "1111" },
                    "tokenizationData": { "type": tokenizing.as_str().unwrap_or("PAYMENT_GATEWAY"), "token": token_text },
                },
            });
            let card =
                json!({ "paymentMethodDescription": "Visa •••• 1111", "cardNetwork": "VISA" });
            (details, card)
        };
        let o = s.options.clone();
        let response = json!({
            "requestId": s.request_id,
            "methodName": s.method,
            "details": details,
            "shippingAddress": if o["requestShipping"] == true { s.address.clone() } else { Json::Null },
            "shippingOption": if o["requestShipping"] == true { s.option.clone() } else { Json::Null },
            "payerName": (o["requestPayerName"] == true).then_some(PAYER.0),
            "payerEmail": (o["requestPayerEmail"] == true).then_some(PAYER.1),
            "payerPhone": (o["requestPayerPhone"] == true).then_some(PAYER.2),
        });
        self.sheet_mut(token).card = card;
        response
    }

    /// `tap @t cancel`: `AbortError` before any result, else the last result.
    fn sheet_cancelled(&mut self, token: u64) {
        let s = self.sheet_mut(token);
        match s.phase {
            Phase::Authorizing => self.reopen(token),
            Phase::Abandoned if s.answered => {
                let hold = s.hold;
                self.retire_hold(hold);
                self.extensions.sheets.open.remove(&token);
            }
            Phase::Abandoned => self.sheet_dropped(token, "closed after its call ended"),
            _ if s.last.is_some() => {
                let answer = self.payment_result(token);
                self.sheet_ends(token, answer);
            }
            _ => {
                let id = s.request_id.clone();
                self.log(format!("payment {id}: cancelled"));
                self.sheet_ends(token, json!({ "error": "AbortError" }));
            }
        }
    }

    /// Contract sends let go (§9): held while authorizing, closed where the
    /// platform can close the sheet, else abandoned.
    pub(super) fn sheets_let_go(&mut self) {
        let gone: Vec<u64> = (self.extensions.held.iter())
            .filter(|h| h.reply.is_none() && self.extensions.sheets.open.contains_key(&h.token))
            .filter(|h| !self.pending.iter().any(|p| p.continuation == Some(h.token)))
            .filter(|h| !self.extensions.sheets.open[&h.token].let_go)
            .map(|h| h.token)
            .collect();
        let target = self.host_target().to_owned();
        for token in gone {
            let s = self.sheet_mut(token);
            s.let_go = true;
            let id = s.request_id.clone();
            match s.phase {
                Phase::Authorizing => {
                    self.log(format!("payment {id}: its call was let go while authorizing; held until the handler answers"));
                }
                Phase::Abandoned => {}
                _ => {
                    let closes = matches!(target.as_str(), "ios" | "macos")
                        || (target == "web" && !s.retrying);
                    if closes {
                        self.sheet_dropped(token, "closed, its call was let go");
                    } else {
                        s.phase = Phase::Abandoned;
                        s.asking = None;
                        self.log(format!("payment {id}: abandoned, its call was let go; the sheet stays until it is closed"));
                        self.reopen(token);
                    }
                }
            }
        }
    }

    /// Whether the held call `token`, let go, is kept for its sheet.
    pub(super) fn sheet_kept(&self, token: u64) -> bool {
        self.extensions
            .sheets
            .open
            .get(&token)
            .is_some_and(|s| s.let_go)
    }

    /// Handlers past their budget on the session's clock (D5).
    pub(super) fn sheet_deadlines(&mut self, now_ms: f64) {
        self.finals_due(now_ms);
        let due: Vec<u64> = (self.extensions.sheets.open.iter())
            .filter(|(_, s)| s.asking.as_ref().is_some_and(|a| a.ends <= now_ms))
            .map(|(t, _)| *t)
            .collect();
        for token in due {
            let Some(asking) = self.sheet_mut(token).asking.take() else {
                continue;
            };
            let id = self.sheet_mut(token).request_id.clone();
            self.log(format!(
                "payments: {} for {id}: past its budget",
                asking.handler
            ));
            let record = json!({ "name": asking.handler, "ms": now_ms - asking.started, "answer": "timeout" });
            self.sheet_mut(token).handled.push(record);
            match asking.ask.as_str() {
                "authorize" => {
                    self.sheet_result(token, "fail", Some("authorize-timeout"), Json::Null)
                }
                "merchantValidation" => {
                    let why = "merchant validation is past its budget";
                    self.sheet_ends(token, json!({ "error": "OperationError", "message": why }));
                }
                _ => {
                    self.sheet_mut(token).errors = json!({ "error": "unavailable" });
                    self.reopen(token);
                }
            }
        }
    }

    /// The sheet's entry in `state.extensions` (§9's disclosure), when the
    /// call held under `token` is a sheet's.
    pub(super) fn sheet_entry(&self, token: u64, outcome: &str) -> Option<Json> {
        let s = self.extensions.sheets.open.get(&token)?;
        Some(json!({
            "name": "showPaymentRequest",
            "requestId": s.request_id,
            "method": s.method,
            "total": amount_text(&s.details["total"]["amount"]),
            "origin": s.origin.word(),
            "outcome": outcome,
            "handlers": s.handled,
        }))
    }
}

/// `USD 34.00`.
fn amount_text(amount: &Json) -> String {
    format!(
        "{} {}",
        amount["currency"].as_str().unwrap_or_default(),
        amount["value"].as_str().unwrap_or_default()
    )
}
