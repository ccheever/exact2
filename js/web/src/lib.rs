//! Browser-owned data module, without a JavaScript engine in wasm.
//! @ref LLP 1027 D6 / LLP 1027.000 D3. Browser microtask checkpoints and
//! HTTP requests both travel through the runner's stale-safe ticket path.
use exact_js_value::{reply_from_json_slice, to_json, Shape};
use exact_plan::{Plan, Value};
pub use exact_runner::Placement;
use exact_runner::{Answer, DataError, DataSource, Dispatch, Outcome, Request, Store};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

#[cfg(target_arch = "wasm32")]
fn call(input: Json) -> Result<Vec<u8>, DataError> {
    #[link(wasm_import_module = "exact_js")]
    extern "C" {
        fn call(op: u32, ptr: *mut u8, len: usize) -> usize;
    }
    let mut bytes = serde_json::to_vec(&input).map_err(|e| unavailable(e.to_string()))?;
    // The import reads only this owned buffer; the second call fills a new
    // owned buffer. It never re-enters the host's borrowed input/output cell.
    let count = unsafe { call(0, bytes.as_mut_ptr(), bytes.len()) };
    if count > 32 * 1024 * 1024 {
        return Err(unavailable("browser module response exceeds 32 MiB"));
    }
    let mut output = vec![0; count];
    unsafe {
        call(1, output.as_mut_ptr(), count);
    }
    Ok(output)
}
#[cfg(not(target_arch = "wasm32"))]
fn call(_: Json) -> Result<Vec<u8>, DataError> {
    Err(unavailable("the browser module executor requires wasm32"))
}

/// One private module environment selected by the browser loader.
pub struct Module {
    app: String,
    grants: String,
    revision: String,
    id: u64,
    ready: bool,
    signatures: HashMap<String, (Vec<Shape>, Shape)>,
    waiting: HashMap<String, bool>, // true: a browser checkpoint; false: HTTP
    /// Where the loader runs this module's turns (LLP 1027.002 D1): the
    /// page's private iframe realm, or a dedicated Worker.
    placement: Placement,
}
impl Module {
    /// Construct from binary-admitted identity/grants and baked HBC digest.
    /// No browser call or app code executes during construction.
    pub fn new(app: &str, grants: &str, revision: &str) -> Self {
        Self {
            app: app.into(),
            grants: grants.into(),
            revision: revision.into(),
            id: 0,
            ready: false,
            signatures: HashMap::new(),
            waiting: HashMap::new(),
            placement: Placement::Main,
        }
    }

    /// This module, placed: the loader prepares its realm as a dedicated
    /// Worker for `Worker`. Carried by replacements, never changed by one.
    pub fn placed(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }
    fn invoke(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Option<Outcome>,
    ) -> Result<Answer, DataError> {
        if !self.ready {
            return Err(unavailable("browser module not loaded"));
        }
        let (params, _) = self
            .signatures
            .get(source)
            .ok_or_else(|| DataError::UnknownSource(source.into()))?;
        if params.len() != args.len() {
            return Err(DataError::BadArguments(format!(
                "expected {} arguments, got {}",
                params.len(),
                args.len()
            )));
        }
        let args = args
            .iter()
            .zip(params)
            .enumerate()
            .map(|(i, (value, shape))| {
                to_json(value, shape).map_err(|_| DataError::BadArguments(format!("argument {i}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot: Vec<_> = store
            .snapshot()
            .into_iter()
            .filter(|(name, _)| !name.starts_with(Store::KEPT))
            .collect();
        let key = json!([source, args]).to_string();
        let mut input = json!({"op":"answer", "id":self.id, "source":source, "args":args, "store":snapshot, "grants":store.granted()});
        let response = match outcome {
            None => call(input)?,
            Some(outcome) => {
                let checkpoint = self
                    .waiting
                    .remove(&key)
                    .ok_or_else(|| unavailable("reply for an answer not in flight"))?;
                if checkpoint {
                    match outcome {
                        Outcome::Storage(_) => {
                            return Err(unavailable(
                                "storage result supplied to JavaScript checkpoint",
                            ))
                        }
                        Outcome::Response(response) => response.body,
                        Outcome::Failed { message, .. } => return Err(unavailable(message)),
                    }
                } else {
                    input["op"] = "resume".into();
                    input["outcome"] = match outcome {
                        Outcome::Storage(_) => {
                            return Err(unavailable("storage result supplied to fetch"))
                        }
                        Outcome::Response(r) => {
                            json!({"response":{"status":r.status,"headers":r.headers,"body":String::from_utf8_lossy(&r.body),"bodyBase64":exact_runner::agent::base64(&r.body)}})
                        }
                        Outcome::Failed { kind, message } => {
                            json!({"failed":{"kind":format!("{kind:?}"),"message":message}})
                        }
                    };
                    call(input)?
                }
            }
        };
        self.step(store, source, key, &response)
    }

    fn step(
        &mut self,
        store: &mut Store,
        source: &str,
        key: String,
        bytes: &[u8],
    ) -> Result<Answer, DataError> {
        let (_, result) = &self.signatures[source];
        let reply = reply_from_json_slice(bytes, result).map_err(|e| unavailable(e.to_string()))?;
        let response = reply.fields;
        if let Some(token) = response["continuation"].as_u64() {
            self.waiting.insert(key, true);
            return Ok(Answer::Later(Request::continuation(token)));
        }
        if response["externalRead"] == true {
            store.observe_external_read();
        }
        if let Some(reads) = response["reads"].as_array() {
            for name in reads {
                if let Some(name) = name.as_str() {
                    store.get(name);
                }
            }
        }
        // Match Hermes's store effects even when the answer subsequently
        // throws or fails shape checking. Transaction rollback is the runner's.
        if let Some(writes) = response["writes"].as_array() {
            for write in writes {
                let name = write[0]
                    .as_str()
                    .ok_or_else(|| unavailable("invalid store write"))?;
                if let Some(value) = write[1].as_str() {
                    store.set(name, value)?;
                } else if write[1].is_null() {
                    store.forget(name)?;
                } else {
                    return Err(unavailable("invalid store value"));
                }
            }
        }
        if let Some(error) = response["error"].as_str() {
            return Err(unavailable(error));
        }
        if response["tag"] == 2 {
            let message = response["message"].as_str().unwrap_or("").to_string();
            return Err(match response["kind"].as_str() {
                Some("UnknownSource") => DataError::UnknownSource(message),
                Some("BadArguments") => DataError::BadArguments(message),
                _ => unavailable(message),
            });
        }
        let answer =
            if response["tag"] == 1 {
                let r = &response["request"];
                let mut request = Request::get(
                    r["url"]
                        .as_str()
                        .ok_or_else(|| unavailable("fetch has no URL"))?,
                );
                request.method = r["method"]
                    .as_str()
                    .ok_or_else(|| unavailable("fetch has no method"))?
                    .into();
                request.headers = serde_json::from_value(r["headers"].clone())
                    .map_err(|e| unavailable(e.to_string()))?;
                request.body = r["body"].as_str().unwrap_or("").as_bytes().to_vec();
                self.waiting.insert(key, false);
                Answer::Later(request)
            } else if response["tag"] == 0 {
                Answer::Now(reply.value.map_err(|e| {
                    unavailable(format!("`{source}` answered outside its shape: {e}"))
                })?)
            } else {
                return Err(unavailable("browser module returned no answer tag"));
            };
        Ok(answer)
    }
}

impl DataSource for Module {
    fn app_id(&self) -> &str {
        &self.app
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn activate(&mut self) -> Result<(), DataError> {
        let response = call(
            json!({"op":"activate", "id":self.id, "appId":self.app, "grants":self.grants, "revision":self.revision, "placement":self.placement.name()}),
        )?;
        let response: Json =
            serde_json::from_slice(&response).map_err(|e| unavailable(e.to_string()))?;
        if response["ok"] != true {
            return Err(unavailable(
                response["error"]
                    .as_str()
                    .unwrap_or("browser module unavailable"),
            ));
        }
        self.ready = true;
        Ok(())
    }
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        let meta: Json = serde_json::from_str(receipt).map_err(|e| unavailable(e.to_string()))?;
        let id: u64 = serde_json::from_slice(&module).map_err(|e| unavailable(e.to_string()))?;
        if meta["version"] != 1
            || meta["abi"] != 1
            || meta["appId"] != self.app
            || meta["grants"].as_str().map(str::trim) != Some(self.grants.trim())
            || meta["plan"]["bytes"].as_u64() != Some(plan.len() as u64)
            || meta["plan"]["sha256"] != format!("{:x}", Sha256::digest(plan))
        {
            return Err(unavailable(
                "module pair changes admitted identity/grants or mismatches its plan",
            ));
        }
        let revision = meta["module"]["sha256"]
            .as_str()
            .ok_or_else(|| unavailable("missing module identity"))?;
        let mut next = Self::new(&self.app, &self.grants, revision);
        next.id = id;
        next.placement = self.placement;
        Ok(next)
    }

    fn placement(&self) -> Placement {
        self.placement
    }

    /// The turn takes its snapshot now, from the store as committed (LLP
    /// 1027.002 D3, change 1), scoped to this module's own names; the
    /// loader's registry runs the token its way.
    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        let granted = Store::new(&self.grants, []).granted().to_vec();
        let snapshot: Vec<_> = store
            .snapshot()
            .into_iter()
            .filter(|(name, _)| !name.starts_with(Store::KEPT) && granted.iter().any(|g| g == name))
            .collect();
        match call(
            json!({"op":"dispatch", "id":self.id, "token":token, "store":snapshot, "grants":granted}),
        ) {
            Ok(response)
                if serde_json::from_slice::<Json>(&response)
                    .is_ok_and(|value| value["ok"] == true) =>
            {
                Dispatch::Host(token)
            }
            _ => Dispatch::Missing,
        }
    }

    fn discard(&mut self, token: u64) {
        let _ = call(json!({"op":"discard", "id":self.id, "token":token}));
    }
    fn bind(&mut self, plan: &Plan) {
        self.signatures.clear();
        for row in &plan.sources {
            let params = plan.source_params
                [row.params.start as usize..(row.params.start + row.params.len) as usize]
                .iter()
                .map(|p| Shape::from_plan(plan, p.ty))
                .collect::<Result<Vec<_>, _>>();
            if let (Ok(params), Ok(result)) = (params, Shape::from_plan(plan, row.ty)) {
                self.signatures
                    .insert(plan.str(row.name).into(), (params, result));
            }
        }
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match self.invoke(
            &mut Store::new(&self.grants.clone(), []),
            source,
            args,
            None,
        )? {
            Answer::Now(value) => Ok(value),
            Answer::Later(_) => Err(unavailable("browser answer requires a host continuation")),
        }
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.invoke(store, source, args, None)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.invoke(store, source, args, Some(outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_storage_observations_survive_refusal_without_a_secret_read() {
        let mut module = Module::new("test", "", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::Unit));
        let mut store = Store::new("", []);
        assert!(module.step(&mut store, "source", "key".into(),
            br#"{"tag":2,"kind":"Unavailable","message":"denied","externalRead":true,"reads":[]}"#).is_err());
        assert_eq!(store.reads(), 1);
        assert!(store.snapshot().is_empty());
        assert!(store.take_writes().is_empty());
    }

    #[test]
    fn failed_answers_preserve_store_effects_for_the_runner_to_decide() {
        let mut module = Module::new("test", "secret.keep token", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::Unit));
        for error in [
            json!({"tag":2,"kind":"Unavailable","message":"failed"}),
            json!({"error":"serialization failed"}),
        ] {
            let mut store = Store::new("secret.keep token", []);
            let mut response = error;
            response["writes"] = json!([["token", "changed"]]);
            assert!(module
                .step(
                    &mut store,
                    "source",
                    "key".into(),
                    &serde_json::to_vec(&response).unwrap()
                )
                .is_err());
            assert_eq!(store.get("token"), Some("changed"));
        }
    }

    #[test]
    fn shaped_answers_decode_directly_and_shape_errors_keep_store_effects() {
        let mut module = Module::new("test", "secret.keep token", "revision");
        module
            .signatures
            .insert("source".into(), (vec![], Shape::Number));
        let mut store = Store::new("secret.keep token", []);
        let answer = module
            .step(
                &mut store,
                "source",
                "key".into(),
                br#"{"tag":0,"value":false,"value":-0,"writes":[["token","ok"]]}"#,
            )
            .unwrap();
        let Answer::Now(Value::Number(number)) = answer else {
            panic!("expected a numeric answer");
        };
        assert_eq!(number.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(store.get("token"), Some("ok"));
        assert!(module
            .step(
                &mut store,
                "source",
                "key".into(),
                br#"{"tag":0,"value":false,"writes":[["token","before-error"]]}"#,
            )
            .is_err());
        assert_eq!(store.get("token"), Some("before-error"));
    }
}
